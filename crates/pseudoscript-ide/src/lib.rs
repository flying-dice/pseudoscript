//! `pseudoscript-ide` — the single browser wasm for the `PseudoScript` web IDE.
//!
//! One crate, one wasm artifact, one surface: [`IdeSession`]. It holds the
//! canonical workspace state — the consumer's own modules and the resolved
//! dependency externals (`LANG.md` §8.3) — and answers every query the IDE
//! drives by delegating to the toolchain crates consumed as typed rlibs:
//! [`pseudoscript_lsp_core`] (the same handlers the stdio server uses),
//! [`pseudoscript_emit`] (diagrams), [`pseudoscript_doc`] (the doc site),
//! [`pseudoscript_format`], and [`pseudoscript_model`]/[`pseudoscript_syntax`].
//!
//! The boundary is **typed**: the DTOs below derive [`tsify_next::Tsify`], so
//! wasm-bindgen emits the real TS interfaces and values cross as objects — no
//! hand-written types, no `JSON.parse`. The one exception is the render IR
//! [`Scene`], an opaque JSON `String` the canvas reads structurally.
//!
//! JavaScript drives the session through two ports — the file system pushes
//! modules in (`mount`/`set_source`), the editor pulls answers out
//! (`diagnostics`/`completion`/`hover`/…).

use std::collections::{BTreeMap, HashMap};

use pseudoscript_doc::{
    DocConfig, DocGroup, DocPage, RenderError, RenderedPage, SsrEngine, Theme, ssr_bundle,
    try_render_site_with,
};
use pseudoscript_emit::{
    Scene, View, layout_c4_scene, layout_data_scene, layout_feature_scene, layout_sequence_scene,
    project, project_symbol,
};
use pseudoscript_format::format as format_source;
use pseudoscript_lsp_core::{analysis, complete, convert, refs, semantic};
use pseudoscript_model::{
    Graph, Workspace, WorkspaceModule,
    ast::Module as AstModule,
    check as check_source,
    deps::{DepFile, resolve_dependency_modules},
    resolve::resolve_at,
    static_diagnostics,
};
use pseudoscript_syntax::{
    Diagnostic as SynDiagnostic, LineIndex, Severity, TokenKind, parse as parse_source, tokenize,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, field};
use tsify_next::Tsify;
use wasm_bindgen::prelude::*;

// ===========================================================================
// Boundary DTOs — the typed contract. Field names mirror the wire shapes the
// IDE already consumes; `tsify` turns each into a TS interface.
// ===========================================================================

/// One `(fqn, source)` module — the file-system port's unit. Input to `mount`
/// and `set_source`, output of [`IdeSession::dependency_modules`].
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Module {
    pub fqn: String,
    pub source: String,
}

/// One diagnostic: its byte span plus 1-based line/column endpoints, so the
/// editor can place a squiggle without re-indexing the source.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
    pub code: Option<String>,
    pub start: u32,
    pub end: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// One module's diagnostics in the workspace check result.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ModuleResult {
    pub fqn: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// One completion item: the label, the integer LSP `CompletionItemKind` (the
/// editor maps it to an icon), and an optional detail.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Completion {
    pub label: String,
    pub kind: u32,
    pub detail: Option<String>,
}

/// Markdown hover content — the editor reads `contents.value`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Hover {
    pub contents: MarkupContent,
}

/// The `{ kind, value }` of an LSP `MarkupContent`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct MarkupContent {
    pub kind: String,
    pub value: String,
}

/// The result of find-usages: the resolved symbol plus every occurrence.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct References {
    pub fqn: String,
    pub title: String,
    pub occurrences: Vec<Occurrence>,
}

/// One find-usages hit: its module `fqn`, 1-based span, the trimmed source line
/// for a preview, and `decl` marking the declaration site. `match_start`/
/// `match_end` are char offsets into `text` bounding the symbol token.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Occurrence {
    pub fqn: String,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub text: String,
    pub match_start: u32,
    pub match_end: u32,
    pub decl: bool,
}

/// One occurrence the host chose to rename, keyed by module `fqn` and the
/// 1-based `line`/`col` [`References`] reported. Input to `rename_apply`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct RenameSelection {
    pub fqn: String,
    pub line: u32,
    pub col: u32,
}

/// One module's rewritten source after a rename — output of `rename_apply`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct RenamedSource {
    pub fqn: String,
    pub source: String,
}

/// One declared node (or `feature`), for the outline / diagram target picker.
/// `line`/`col` are the 1-based position of the name in its own module;
/// `parent` is the FQN of the enclosing node (the C4 containment, §6) or `null`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct OutlineNode {
    pub fqn: String,
    pub name: String,
    pub kind: String,
    pub triggered: bool,
    pub line: u32,
    pub col: u32,
    pub parent: Option<String>,
    pub summary: Option<String>,
}

/// The kind of construct a fold covers (§3.5/§5.1), so the editor can pick a
/// default fold state per kind — collapse `member` impl blocks on open, leave
/// the structural `node` bodies expanded.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "lowercase")]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub enum FoldKind {
    Node,
    Member,
    Data,
    Block,
}

/// A foldable region, 0-based lines — the editor folds these instead of
/// brace-matching in JS — tagged with the kind of construct it covers.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct FoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: FoldKind,
}

/// AST-aware semantic tokens: the delta-encoded flat `data` array over UTF-16
/// positions, identical to the stdio server's `semanticTokens/full` response.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct SemanticTokens {
    pub data: Vec<u32>,
}

/// The `[doc]` table parsed from a `pds.toml` for the host: the sidebar groups
/// and their page entries (no content — the host loads the files the manifest
/// names, then hands the assembled config to `render_doc_site`).
#[derive(Debug, Default, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DocManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default)]
    pub sidebar: Vec<DocManifestGroup>,
}

/// One `[[doc.sidebar]]` group in the manifest. `title` is required (a group is
/// a named heading); `items` defaults to empty.
#[derive(Debug, Default, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DocManifestGroup {
    pub title: String,
    #[serde(default)]
    pub items: Vec<DocManifestItem>,
}

/// One `{ title, path }` page entry in the manifest — both required.
#[derive(Debug, Default, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DocManifestItem {
    pub title: String,
    pub path: String,
}

/// One rendered site file returned by `render_doc_site`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct RenderedFile {
    pub path: String,
    pub contents: String,
}

/// The host's documentation config for `render_doc_site`: site name, optional
/// theme word (`dark`/`light`, default `dark`), optional logo path, and the
/// authored doc groups with their pages' already-loaded Markdown `content`.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DocConfigInput {
    pub name: String,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub docs: Vec<DocGroupInput>,
}

/// One doc group in [`DocConfigInput`]: a heading and its pages (with content).
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DocGroupInput {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<DocItemInput>,
}

/// One page in a [`DocGroupInput`]: its title, source path, and Markdown body.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DocItemInput {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub content: String,
}

/// One vendored git-dependency file for `dependency_modules`: its
/// `pds_modules/` slug, its FQN within the dependency workspace (the host's
/// path→FQN derivation), and its source.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct VendoredInput {
    pub slug: String,
    pub fqn: String,
    pub source: String,
}

/// One local-source dependency file for `dependency_modules`: the dependency
/// name (ADR-026), its FQN within the dependency workspace, and its source.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct LocalInput {
    pub name: String,
    pub fqn: String,
    pub source: String,
}

// ===========================================================================
// IdeSession — the one wasm surface, with one cache.
// ===========================================================================

/// One module's cached parse — kept so an edit re-parses only the edited module.
struct Parsed {
    source: String,
    ast: AstModule,
    diagnostics: Vec<SynDiagnostic>,
}

impl Parsed {
    fn new(source: String) -> Self {
        let parsed = parse_source(&source);
        Self {
            source,
            ast: parsed.ast,
            diagnostics: parsed.diagnostics,
        }
    }
}

/// The derived artifacts, rebuilt once per change and read by every query: the
/// resolved [`Workspace`] (language intelligence) and the [`Graph`] projected
/// from it (outline, diagrams, docs). The graph is built from the workspace, so
/// both come from one rebuild and share one invalidation.
struct Built {
    workspace: Workspace,
    graph: Graph,
}

/// The IDE session: the workspace state plus every language, diagram, and doc
/// query over it. The host holds one per open workspace.
///
/// One cache, one staleness story: `modules` caches each module's parse (so an
/// edit re-parses only that module), and `built` memoises the resolved workspace
/// + graph. Any mutation clears `built`; the next query rebuilds it once and the
/// rest of a query burst between keystrokes reads it — nothing unchanged is ever
/// re-parsed or re-resolved.
#[wasm_bindgen]
#[derive(Default)]
pub struct IdeSession {
    /// The consumer's own modules, by FQN; an open buffer overlays via `set_source`.
    modules: BTreeMap<String, Parsed>,
    /// Dependency modules resolved as externals (§8.3), dependency-name-prefixed.
    externals: Vec<WorkspaceModule>,
    /// The resolved workspace + graph, or `None` when stale (after any mutation).
    built: Option<Built>,
}

#[wasm_bindgen]
impl IdeSession {
    /// A new, empty session.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        install_tracing();
        Self::default()
    }

    // ---- file-system port: push modules + edits in ------------------------

    /// Load the workspace: re-parse every module, replace the externals, and
    /// invalidate the built workspace/graph.
    pub fn mount(&mut self, modules: Vec<Module>, externals: Vec<Module>) {
        self.modules = modules
            .into_iter()
            .map(|m| (m.fqn, Parsed::new(m.source)))
            .collect();
        self.externals = externals.into_iter().map(into_workspace_module).collect();
        self.built = None;
        debug!(
            modules = self.modules.len(),
            externals = self.externals.len(),
            "mount"
        );
    }

    /// Apply an edit: re-parse only this module and invalidate the built
    /// workspace/graph. An unchanged buffer (a defensive re-push) is a no-op, so
    /// the cache stays warm for the query burst that follows.
    pub fn set_source(&mut self, fqn: &str, text: &str) {
        if self.modules.get(fqn).is_some_and(|m| m.source == text) {
            return;
        }
        self.modules
            .insert(fqn.to_owned(), Parsed::new(text.to_owned()));
        self.built = None;
        debug!(fqn, bytes = text.len(), "set_source");
    }

    // ---- editor port: language queries over held state --------------------

    /// Workspace-wide diagnostics, each module's problems attributed to it and
    /// checked against the dependency externals so a `dep::module::Node`
    /// reference resolves (§8.3).
    #[must_use]
    pub fn diagnostics(&mut self) -> Vec<ModuleResult> {
        let _span = tracing::debug_span!(
            "diagnostics",
            modules = field::Empty,
            problems = field::Empty
        )
        .entered();
        let t0 = now_ms();
        self.ensure_built();
        let ws = self.workspace();
        let results: Vec<ModuleResult> = self
            .modules
            .iter()
            .map(|(fqn, m)| {
                let mut diagnostics = m.diagnostics.clone();
                diagnostics.extend(static_diagnostics(ws, fqn));
                ModuleResult {
                    fqn: fqn.clone(),
                    diagnostics: enrich(&diagnostics, &m.source),
                }
            })
            .collect();
        let problems: usize = results.iter().map(|r| r.diagnostics.len()).sum();
        let span = tracing::Span::current();
        span.record("modules", results.len());
        span.record("problems", problems);
        debug!(
            modules = results.len(),
            problems,
            ms = elapsed_ms(t0),
            "diagnostics"
        );
        results
    }

    /// Completions at byte `offset` in module `fqn`, resolved across the
    /// workspace and its externals.
    #[must_use]
    pub fn completion(&mut self, fqn: &str, offset: u32) -> Vec<Completion> {
        // `#[instrument]` can't see `self` through `#[wasm_bindgen]`, so the span
        // is opened by hand; it closes (and times) when `_span` drops.
        let _span = tracing::debug_span!("completion", fqn, offset, count = field::Empty).entered();
        let t0 = now_ms();
        self.ensure_built();
        let src = self.modules.get(fqn).map_or("", |m| m.source.as_str());
        let ws = self.workspace();
        let items = complete_with(ws, fqn, src, offset);
        tracing::Span::current().record("count", items.len());
        debug!(
            fqn,
            offset,
            count = items.len(),
            ms = elapsed_ms(t0),
            "completion"
        );
        items
    }

    /// Markdown hover for the symbol at byte `offset` in module `fqn`.
    #[must_use]
    pub fn hover(&mut self, fqn: &str, offset: u32) -> Option<Hover> {
        let _span = tracing::debug_span!("hover", fqn, offset, resolved = field::Empty).entered();
        let t0 = now_ms();
        self.ensure_built();
        let src = self.modules.get(fqn).map_or("", |m| m.source.as_str());
        let ws = self.workspace();
        let hit = hover_with(ws, fqn, src, offset);
        tracing::Span::current().record("resolved", hit.is_some());
        debug!(
            fqn,
            offset,
            resolved = hit.is_some(),
            ms = elapsed_ms(t0),
            "hover"
        );
        hit
    }

    /// The FQN of the declaration of the symbol at `offset` in module `fqn`, for
    /// go-to-definition; `None` when the cursor rests on no resolvable symbol.
    #[must_use]
    pub fn definition(&mut self, fqn: &str, offset: u32) -> Option<String> {
        self.ensure_built();
        let src = self.modules.get(fqn).map_or("", |m| m.source.as_str());
        let ws = self.workspace();
        definition_with(ws, fqn, src, offset)
    }

    /// Every occurrence of the symbol at `offset` in module `fqn` across the
    /// workspace — find-usages. `None` when the cursor rests on no symbol.
    #[must_use]
    pub fn references(&mut self, fqn: &str, offset: u32) -> Option<References> {
        self.ensure_built();
        let pairs = self.module_pairs();
        let src = self.modules.get(fqn).map_or("", |m| m.source.as_str());
        let ws = self.workspace();
        references_with(ws, &pairs, fqn, src, offset)
    }

    /// Renames the symbol at `offset` in module `fqn` to `new_name`, applying
    /// only the `selected` occurrences (by their 1-based `line`/`col`). Returns
    /// the new full source of every module that changed.
    ///
    /// # Errors
    /// Returns an error when `new_name` is not a valid identifier.
    // `selected` is owned because wasm-bindgen yields owned `Vec<T>` across the
    // ABI (no borrowing a JS array); `rename_at` only borrows it.
    #[allow(clippy::needless_pass_by_value)]
    pub fn rename_apply(
        &mut self,
        fqn: &str,
        offset: u32,
        new_name: &str,
        selected: Vec<RenameSelection>,
    ) -> Result<Vec<RenamedSource>, JsError> {
        self.ensure_built();
        let pairs = self.module_pairs();
        let src = self.modules.get(fqn).map_or("", |m| m.source.as_str());
        let ws = self.workspace();
        rename_with(ws, &pairs, fqn, src, offset, new_name, &selected).map_err(|e| JsError::new(&e))
    }

    /// The nodes declared across the workspace, for a diagram's target picker.
    #[must_use]
    pub fn outline(&mut self) -> Vec<OutlineNode> {
        self.ensure_built();
        let indices: HashMap<&str, LineIndex> = self
            .modules
            .iter()
            .map(|(fqn, m)| (fqn.as_str(), LineIndex::new(&m.source)))
            .collect();
        let graph = self.graph();
        outline_nodes(graph, |module, offset| {
            indices
                .get(module)
                .map_or((1, 1), |index| index.line_col(offset))
        })
    }

    /// Parse + static-check a single `source` buffer — the editor's per-keystroke
    /// lint path (one module, no workspace context, so cheaper than
    /// [`Self::diagnostics`]), independent of held state.
    #[must_use]
    pub fn check(&self, source: &str) -> Vec<Diagnostic> {
        enrich(&check_source(source), source)
    }

    /// Semantic tokens for a single `source` buffer (editor-local).
    #[must_use]
    pub fn semantic_tokens(&self, source: &str) -> SemanticTokens {
        to_semantic_tokens(&semantic::semantic_tokens(source))
    }

    /// Foldable regions of a single `source` buffer (editor-local), each tagged
    /// with its construct kind. 0-based lines; single-line spans are dropped (an
    /// editor cannot fold them).
    #[must_use]
    pub fn folding_ranges(&self, source: &str) -> Vec<FoldingRange> {
        let index = LineIndex::new(source);
        pseudoscript_model::folding_ranges(source)
            .into_iter()
            .map(|r| FoldingRange {
                start_line: index.line_col(r.start).0 - 1,
                end_line: index.line_col(r.end).0 - 1,
                kind: fold_kind(r.kind),
            })
            .filter(|r| r.end_line > r.start_line)
            .collect()
    }

    /// Canonical formatting of a single `source` buffer.
    ///
    /// # Errors
    /// Returns an error when `source` does not parse.
    pub fn format(&self, source: &str) -> Result<String, JsError> {
        format_source(source).map_err(|e| JsError::new(&e.to_string()))
    }

    // ---- diagram projection (Scene = opaque JSON passthrough) -------------

    /// Projects a diagram `view` over the held workspace and returns the
    /// [`Scene`] as JSON. `view` is `context`/`container`/`component`/`sequence`;
    /// `target` is the boundary or entry FQN (ignored for `context`).
    ///
    /// # Errors
    /// Returns an error for an unknown `view` or a view that cannot be projected.
    pub fn emit_scene(&mut self, view: &str, target: &str) -> Result<String, JsError> {
        let view = view_of(view, target).map_err(|e| JsError::new(&e))?;
        self.ensure_built();
        let graph = self.graph();
        let scene = project(graph, view).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(to_json(&scene))
    }

    /// Projects the fitting diagram for the symbol `fqn` over the held workspace
    /// and returns its [`Scene`] as JSON (the side-panel counterpart of `hover`).
    ///
    /// # Errors
    /// Returns an error for an unknown symbol or one that cannot be projected.
    pub fn symbol_scene(&mut self, fqn: &str) -> Result<String, JsError> {
        self.ensure_built();
        let graph = self.graph();
        let scene = project_symbol(graph, fqn).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(to_json(&scene))
    }

    /// Positions a [`Scene`] (as JSON) into absolute coordinates, returning the
    /// layout as JSON. The two layout shapes are distinguishable by their fields
    /// (`participants` vs `nodes`).
    ///
    /// # Errors
    /// Returns an error for invalid JSON.
    #[allow(clippy::unused_self)]
    pub fn layout_scene(&self, scene_json: &str) -> Result<String, JsError> {
        layout_of(scene_json).map_err(|e| JsError::new(&e))
    }

    // ---- project + docs ----------------------------------------------------

    /// Resolves the consumer workspace's direct dependencies (§8.3) into
    /// dependency-name-prefixed modules — the externals `mount` takes. `lock` is
    /// the consumer's `pds.lock` (blank when absent); `vendored`/`local` are the
    /// files the host read for vendored git deps and local-source deps (ADR-026).
    ///
    /// # Errors
    /// Returns an error when `lock` is present but not valid TOML.
    #[allow(clippy::unused_self)]
    pub fn dependency_modules(
        &self,
        lock: &str,
        vendored: Vec<VendoredInput>,
        local: Vec<LocalInput>,
    ) -> Result<Vec<Module>, JsError> {
        let vendored: Vec<(String, DepFile)> = vendored
            .into_iter()
            .map(|v| (v.slug, DepFile::new(v.fqn, v.source)))
            .collect();
        let local: Vec<(String, DepFile)> = local
            .into_iter()
            .map(|l| (l.name, DepFile::new(l.fqn, l.source)))
            .collect();
        let modules = resolve_dependency_modules(lock, &vendored, &local)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(modules
            .into_iter()
            .map(|m| Module {
                fqn: m.fqn,
                source: m.source,
            })
            .collect())
    }

    /// Parses a `pds.toml` string into the doc manifest the host needs to build
    /// the sidebar and read its pages. Uses the same `toml` parser as the CLI.
    ///
    /// # Errors
    /// Returns an error when `toml` is not valid TOML of the `[doc]` shape.
    #[allow(clippy::unused_self)]
    pub fn doc_manifest(&self, toml: &str) -> Result<DocManifest, JsError> {
        doc_manifest_of(toml).map_err(|e| JsError::new(&e))
    }

    /// The Svelte SSR bundle (`ssr.js`) the host evaluates in its own JavaScript
    /// engine to define `globalThis.SSR.renderPage` — hand that back to
    /// [`Self::render_doc_site`] as the `render` callback.
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn doc_ssr_bundle(&self) -> String {
        ssr_bundle().to_owned()
    }

    /// Renders the whole documentation site for the held workspace, exactly as
    /// `pds doc` does, driving SSR through the host's JS `render` callback
    /// (`(propsJson) => head/body JSON`, typically `SSR.renderPage`).
    ///
    /// # Errors
    /// Returns an error when a page fails to render (a bundle/engine defect).
    pub fn render_doc_site(
        &mut self,
        config: DocConfigInput,
        render: &js_sys::Function,
    ) -> Result<Vec<RenderedFile>, JsError> {
        let engine = HostEngine {
            render: render.clone(),
        };
        self.ensure_built();
        let graph = self.graph();
        let site = try_render_site_with(graph, &doc_config(config), &engine)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(site
            .files
            .iter()
            .map(|f| RenderedFile {
                path: f.path.clone(),
                contents: f.contents.clone(),
            })
            .collect())
    }
}

impl IdeSession {
    /// Rebuild the resolved workspace + graph from the cached module ASTs if a
    /// mutation cleared them. Lazy, so a burst of queries between keystrokes pays
    /// for one rebuild; the cached per-module parses mean only the edited module
    /// was ever re-parsed.
    fn ensure_built(&mut self) {
        if self.built.is_none() {
            let workspace = Workspace::build_with_externals(
                self.modules
                    .iter()
                    .map(|(fqn, m)| (fqn.clone(), m.ast.clone())),
                self.externals
                    .iter()
                    .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
            );
            let graph = Graph::build(&workspace);
            self.built = Some(Built { workspace, graph });
        }
    }

    /// The resolved workspace. Call [`Self::ensure_built`] first.
    fn workspace(&self) -> &Workspace {
        &self
            .built
            .as_ref()
            .expect("ensure_built ran first")
            .workspace
    }

    /// The projected graph. Call [`Self::ensure_built`] first.
    fn graph(&self) -> &Graph {
        &self.built.as_ref().expect("ensure_built ran first").graph
    }

    /// Every module as an owned `(fqn, source)` pair, for the whole-workspace
    /// scans (references, rename).
    fn module_pairs(&self) -> Vec<(String, String)> {
        self.modules
            .iter()
            .map(|(fqn, m)| (fqn.clone(), m.source.clone()))
            .collect()
    }
}

// ===========================================================================
// Free functions — the logic, host-testable (no `JsError`, which cannot exist
// off-wasm). The `*_with` cores take an already-built `Workspace`/`Graph`, so the
// session (over its cache) and the `*_at`/`*_of` test wrappers (over a fresh
// build) share one implementation.
// ===========================================================================

fn into_workspace_module(m: Module) -> WorkspaceModule {
    WorkspaceModule::new(m.fqn, m.source)
}

/// The source for module `fqn` in `modules`, or empty when not loaded.
#[cfg(test)]
fn source_of<'a>(modules: &'a [WorkspaceModule], fqn: &str) -> &'a str {
    modules
        .iter()
        .find(|m| m.fqn == fqn)
        .map_or("", |m| m.source.as_str())
}

/// Builds a [`Workspace`] from `local` modules plus dependency-prefixed
/// `external` modules (§8.3): cross-workspace references resolve with public-only
/// visibility, and dependency modules are never checked.
#[cfg(test)]
fn build_workspace(local: &[WorkspaceModule], external: &[WorkspaceModule]) -> Workspace {
    Workspace::build_with_externals(
        local
            .iter()
            .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
        external
            .iter()
            .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
    )
}

/// Workspace check, each module's diagnostics attributed to it and enriched
/// against its own source (cross-module errors land on the referrer, §8.2).
#[cfg(test)]
fn check_workspace(
    modules: &[WorkspaceModule],
    externals: &[WorkspaceModule],
) -> Vec<ModuleResult> {
    pseudoscript_model::check_workspace_modules_with_externals(modules, externals)
        .into_iter()
        .map(|module| {
            let diagnostics = enrich(&module.diagnostics, source_of(modules, &module.fqn));
            ModuleResult {
                fqn: module.fqn,
                diagnostics,
            }
        })
        .collect()
}

fn complete_with(ws: &Workspace, fqn: &str, src: &str, offset: u32) -> Vec<Completion> {
    let position = convert::offset_to_position(src, &LineIndex::new(src), offset);
    complete::completion(ws, fqn, src, position)
        .iter()
        .map(to_completion)
        .collect()
}

fn hover_with(ws: &Workspace, fqn: &str, src: &str, offset: u32) -> Option<Hover> {
    let position = convert::offset_to_position(src, &LineIndex::new(src), offset);
    analysis::hover(ws, fqn, src, position).map(|h| to_hover(&h))
}

fn definition_with(ws: &Workspace, fqn: &str, src: &str, offset: u32) -> Option<String> {
    resolve_at(ws, fqn, src, offset).map(|hit| hit.target_fqn)
}

fn references_with(
    ws: &Workspace,
    pairs: &[(String, String)],
    fqn: &str,
    src: &str,
    offset: u32,
) -> Option<References> {
    let target = resolve_at(ws, fqn, src, offset)?;

    // Scan every name-position identifier in the workspace, keeping those that
    // resolve to the same definition (a `::` qualifier names a module, never the
    // symbol).
    let mut occurrences = Vec::new();
    for (mod_fqn, mod_source) in pairs {
        let index = LineIndex::new(mod_source);
        let lines: Vec<&str> = mod_source.lines().collect();
        let tokens = tokenize(mod_source);
        for (i, token) in tokens.iter().enumerate() {
            if token.kind != TokenKind::Ident
                || tokens
                    .get(i + 1)
                    .is_some_and(|t| t.kind == TokenKind::ColonColon)
            {
                continue;
            }
            let Some(hit) = resolve_at(ws, mod_fqn, mod_source, token.span.start) else {
                continue;
            };
            if hit.target_module != target.target_module || hit.target_span != target.target_span {
                continue;
            }
            let (line, col) = index.line_col(token.span.start);
            let (end_line, end_col) = index.line_col(token.span.end);
            // Trim the preview line, then map the token's 1-based byte column into
            // char offsets within the trimmed text so a host can highlight it.
            let raw = lines.get(line as usize - 1).copied().unwrap_or("");
            let lead = raw.len() - raw.trim_start().len();
            let text = raw.trim().to_owned();
            let char_off = |byte: usize| {
                text.get(..byte)
                    .map_or_else(|| text.chars().count(), |s| s.chars().count())
                    as u32
            };
            let match_start = char_off((col as usize).saturating_sub(1).saturating_sub(lead));
            let match_end = char_off((end_col as usize).saturating_sub(1).saturating_sub(lead));
            occurrences.push(Occurrence {
                fqn: mod_fqn.clone(),
                line,
                col,
                end_line,
                end_col,
                text,
                match_start,
                match_end,
                decl: *mod_fqn == target.target_module && token.span == target.target_span,
            });
        }
    }

    Some(References {
        fqn: target.target_fqn,
        title: target.title,
        occurrences,
    })
}

/// Whether `name` is a legal `PseudoScript` identifier (`[A-Za-z_]\w*`).
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn rename_with(
    ws: &Workspace,
    pairs: &[(String, String)],
    fqn: &str,
    src: &str,
    offset: u32,
    new_name: &str,
    selected: &[RenameSelection],
) -> Result<Vec<RenamedSource>, String> {
    if !is_identifier(new_name) {
        return Err(format!("`{new_name}` is not a valid identifier"));
    }
    let chosen: std::collections::HashSet<(&str, u32, u32)> = selected
        .iter()
        .map(|s| (s.fqn.as_str(), s.line, s.col))
        .collect();

    let mut out: Vec<RenamedSource> = Vec::new();
    for file in refs::rename(ws, pairs, fqn, src, offset) {
        let Some((_, mod_source)) = pairs.iter().find(|(f, _)| *f == file.fqn) else {
            continue;
        };
        let index = LineIndex::new(mod_source);
        // Keep the selected occurrences, then apply right-to-left so each splice
        // leaves the earlier byte offsets valid.
        let mut spans: Vec<_> = file
            .spans
            .into_iter()
            .filter(|span| {
                let (line, col) = index.line_col(span.start);
                chosen.contains(&(file.fqn.as_str(), line, col))
            })
            .collect();
        if spans.is_empty() {
            continue;
        }
        spans.sort_by_key(|span| std::cmp::Reverse(span.start));
        let mut source = mod_source.clone();
        for span in spans {
            source.replace_range(span.start as usize..span.end as usize, new_name);
        }
        out.push(RenamedSource {
            fqn: file.fqn,
            source,
        });
    }
    Ok(out)
}

// ---- test wrappers: build a fresh workspace, then call a core. ----

#[cfg(test)]
fn completion_at(
    modules: &[WorkspaceModule],
    externals: &[WorkspaceModule],
    fqn: &str,
    offset: u32,
) -> Vec<Completion> {
    complete_with(
        &build_workspace(modules, externals),
        fqn,
        source_of(modules, fqn),
        offset,
    )
}

#[cfg(test)]
fn hover_at(
    modules: &[WorkspaceModule],
    externals: &[WorkspaceModule],
    fqn: &str,
    offset: u32,
) -> Option<Hover> {
    hover_with(
        &build_workspace(modules, externals),
        fqn,
        source_of(modules, fqn),
        offset,
    )
}

#[cfg(test)]
fn definition_at(
    modules: &[WorkspaceModule],
    externals: &[WorkspaceModule],
    fqn: &str,
    offset: u32,
) -> Option<String> {
    definition_with(
        &build_workspace(modules, externals),
        fqn,
        source_of(modules, fqn),
        offset,
    )
}

#[cfg(test)]
fn references_at(
    modules: &[WorkspaceModule],
    externals: &[WorkspaceModule],
    fqn: &str,
    offset: u32,
) -> Option<References> {
    let pairs: Vec<(String, String)> = modules
        .iter()
        .map(|m| (m.fqn.clone(), m.source.clone()))
        .collect();
    references_with(
        &build_workspace(modules, externals),
        &pairs,
        fqn,
        source_of(modules, fqn),
        offset,
    )
}

#[cfg(test)]
fn outline_of(modules: &[WorkspaceModule]) -> Vec<OutlineNode> {
    let indices: HashMap<&str, LineIndex> = modules
        .iter()
        .map(|m| (m.fqn.as_str(), LineIndex::new(&m.source)))
        .collect();
    let graph = pseudoscript_model::graph(modules);
    outline_nodes(&graph, |module, offset| {
        indices
            .get(module)
            .map_or((1, 1), |index| index.line_col(offset))
    })
}

/// Projects a graph's nodes (and scenarios) to outline entries.
fn outline_nodes(
    graph: &Graph,
    mut line_col: impl FnMut(&str, u32) -> (u32, u32),
) -> Vec<OutlineNode> {
    let mut entries: Vec<OutlineNode> = graph
        .nodes()
        .iter()
        .map(|n| {
            let (line, col) = line_col(&n.module, n.span.start);
            OutlineNode {
                fqn: n.fqn.clone(),
                name: n.name.clone(),
                kind: n.kind.keyword().to_owned(),
                triggered: !n.triggers.is_empty(),
                line,
                col,
                parent: n.parent.clone(),
                summary: n.doc.summary.clone(),
            }
        })
        .collect();
    // Features are not graph nodes (§5.2); project each as a `feature` entry
    // nested under its target node, so the outline lists it as a selectable
    // symbol — the host falls back to a single-lifeline scene on selection.
    entries.extend(graph.scenarios().iter().map(|s| {
        let (line, col) = line_col(&s.module, s.span.start);
        OutlineNode {
            fqn: format!("{}::{}", s.module, s.name),
            name: s.name.clone(),
            kind: "feature".to_owned(),
            triggered: false,
            line,
            col,
            parent: Some(s.target_fqn.clone()),
            summary: None,
        }
    }));
    entries
}

/// Resolves a `view`/`target` pair into an emit [`View`].
fn view_of(view: &str, target: &str) -> Result<View, String> {
    match view {
        "context" => Ok(View::Context),
        "container" => Ok(View::Container {
            of: target.to_owned(),
        }),
        "component" => Ok(View::Component {
            of: target.to_owned(),
        }),
        "sequence" => Ok(View::Sequence {
            entry: target.to_owned(),
        }),
        "data" => Ok(View::Data {
            of: target.to_owned(),
        }),
        "feature" => Ok(View::Feature {
            of: target.to_owned(),
        }),
        other => Err(format!("unknown view `{other}`")),
    }
}

#[cfg(test)]
fn emit_scene_of(modules: &[WorkspaceModule], view: &str, target: &str) -> Result<String, String> {
    let graph = pseudoscript_model::graph(modules);
    let scene = project(&graph, view_of(view, target)?).map_err(|e| e.to_string())?;
    Ok(to_json(&scene))
}

#[cfg(test)]
fn symbol_scene_of(modules: &[WorkspaceModule], fqn: &str) -> Result<String, String> {
    let graph = pseudoscript_model::graph(modules);
    let scene = project_symbol(&graph, fqn).map_err(|e| e.to_string())?;
    Ok(to_json(&scene))
}

fn layout_of(scene_json: &str) -> Result<String, String> {
    let scene: Scene = serde_json::from_str(scene_json).map_err(|e| e.to_string())?;
    match scene {
        Scene::Sequence(seq) => Ok(to_json(&layout_sequence_scene(&seq))),
        Scene::C4(c4) => Ok(to_json(&layout_c4_scene(&c4))),
        Scene::Data(data) => Ok(to_json(&layout_data_scene(&data))),
        Scene::Feature(feature) => Ok(to_json(&layout_feature_scene(&feature))),
    }
}

fn doc_manifest_of(toml_src: &str) -> Result<DocManifest, String> {
    let manifest: ManifestInput = toml::from_str(toml_src).map_err(|e| e.to_string())?;
    Ok(manifest.doc)
}

/// A `pds.toml` for [`doc_manifest_of`]: only its `[doc]` table is read.
#[derive(Deserialize)]
struct ManifestInput {
    #[serde(default)]
    doc: DocManifest,
}

/// Builds a [`DocConfig`] from the host's typed [`DocConfigInput`].
fn doc_config(input: DocConfigInput) -> DocConfig {
    DocConfig {
        theme: if input.theme.as_deref() == Some("light") {
            Theme::Light
        } else {
            Theme::Dark
        },
        name: input.name,
        logo: input.logo,
        docs: input
            .docs
            .into_iter()
            .map(|group| DocGroup {
                title: group.title,
                pages: group
                    .items
                    .into_iter()
                    .map(|item| DocPage {
                        title: item.title,
                        path: item.path,
                        markdown: item.content,
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Backs [`SsrEngine`] with a host JavaScript function: each page's props JSON
/// is handed to `render`, which returns the `{head, body}` JSON string.
struct HostEngine {
    render: js_sys::Function,
}

impl SsrEngine for HostEngine {
    fn render_page(&self, props_json: &str) -> Result<RenderedPage, RenderError> {
        let out = self
            .render
            .call1(&JsValue::NULL, &JsValue::from_str(props_json))
            .map_err(|e| RenderError::Call(format!("{e:?}")))?;
        let json = out.as_string().ok_or_else(|| {
            RenderError::Call("render callback did not return a string".to_owned())
        })?;
        serde_json::from_str(&json).map_err(|e| RenderError::Codec(e.to_string()))
    }
}

// ---- lsp_types → typed DTO mappers (serialize-and-extract, so no lsp-types
// dependency and the wire shape is reproduced exactly) ----------------------

fn to_completion(item: &impl Serialize) -> Completion {
    let v = to_value(item);
    Completion {
        label: v["label"].as_str().unwrap_or_default().to_owned(),
        kind: u32::try_from(v["kind"].as_u64().unwrap_or(0)).unwrap_or(0),
        detail: v["detail"].as_str().map(str::to_owned),
    }
}

fn to_hover(hover: &impl Serialize) -> Hover {
    let v = to_value(hover);
    Hover {
        contents: MarkupContent {
            kind: v["contents"]["kind"]
                .as_str()
                .unwrap_or("markdown")
                .to_owned(),
            value: v["contents"]["value"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        },
    }
}

fn fold_kind(kind: pseudoscript_model::FoldKind) -> FoldKind {
    match kind {
        pseudoscript_model::FoldKind::Node => FoldKind::Node,
        pseudoscript_model::FoldKind::Member => FoldKind::Member,
        pseudoscript_model::FoldKind::Data => FoldKind::Data,
        pseudoscript_model::FoldKind::Block => FoldKind::Block,
    }
}

fn to_semantic_tokens(tokens: &impl Serialize) -> SemanticTokens {
    let v = to_value(tokens);
    let data = v["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_u64)
                .map(|n| u32::try_from(n).unwrap_or(0))
                .collect()
        })
        .unwrap_or_default();
    SemanticTokens { data }
}

/// Enrich each diagnostic's byte span with 1-based line/column endpoints.
fn enrich(diagnostics: &[SynDiagnostic], source: &str) -> Vec<Diagnostic> {
    let index = LineIndex::new(source);
    diagnostics
        .iter()
        .map(|d| {
            let (start_line, start_col) = index.line_col(d.span.start);
            let (end_line, end_col) = index.line_col(d.span.end);
            Diagnostic {
                severity: severity_word(d.severity).to_owned(),
                message: d.message.clone(),
                code: d.code.clone(),
                start: d.span.start,
                end: d.span.end,
                start_line,
                start_col,
                end_line,
                end_col,
            }
        })
        .collect()
}

/// The lowercase word for a severity, matching the LSP/editor vocabulary.
fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

// ---- the crate version + tracing edge -------------------------------------

/// The crate version, for host-side compatibility checks.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Install the panic hook and route `tracing` events to the browser console,
/// once per session lifetime. The console output flows to the dev terminal via
/// Vite's `forwardConsole`. A no-op off wasm.
#[cfg(target_arch = "wasm32")]
fn install_tracing() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        console_error_panic_hook::set_once();
        // Aggressive by default: report every level (down to TRACE) to the
        // browser console, so the whole wasm stack is observable in dev. Vite
        // forwards the console to the dev terminal.
        let mut builder = tracing_wasm::WASMLayerConfigBuilder::new();
        builder.set_max_level(tracing::Level::TRACE);
        tracing_wasm::set_as_global_default_with_config(builder.build());
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn install_tracing() {}

/// Wall-clock milliseconds for coarse query timing in the log events. Uses the
/// browser's `performance.now()`; `0.0` off wasm.
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    0.0
}

/// Milliseconds since `start`, rounded to one decimal for readable log events.
fn elapsed_ms(start: f64) -> f64 {
    ((now_ms() - start) * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `[(fqn, source)]` → owned workspace modules.
    fn mods(list: &[(&str, &str)]) -> Vec<WorkspaceModule> {
        list.iter()
            .map(|(fqn, source)| WorkspaceModule::new((*fqn).to_owned(), (*source).to_owned()))
            .collect()
    }

    /// `ctx`'s source: a person calling a container in `sys`.
    const CTX_SRC: &str =
        "//! ctx\npublic person User {\n  public Use(): void { sys::Web.View() }\n}";
    /// `sys`'s source: a system with one container that has a bodied callable.
    const SYS_SRC: &str = "//! sys\npublic system Shop;\npublic container Web for Shop {\n  public View(): void {}\n}";

    fn workspace() -> Vec<WorkspaceModule> {
        mods(&[("ctx", CTX_SRC), ("sys", SYS_SRC)])
    }

    #[test]
    fn doc_manifest_parses_doc_table_and_sidebar() {
        let toml = r#"
            [doc]
            name = "Banking"

            [[doc.sidebar]]
            title = "Start"
            items = [{ title = "Intro", path = "docs/intro.md" }]
        "#;
        let m = doc_manifest_of(toml).expect("valid manifest");
        assert_eq!(m.name.as_deref(), Some("Banking"));
        assert_eq!(m.sidebar[0].items[0].path, "docs/intro.md");
        assert!(m.logo.is_none());
    }

    #[test]
    fn doc_config_maps_page_content() {
        let input = DocConfigInput {
            name: "X".to_owned(),
            theme: None,
            logo: None,
            docs: vec![DocGroupInput {
                title: "G".to_owned(),
                items: vec![DocItemInput {
                    title: "Intro".to_owned(),
                    path: "docs/intro.md".to_owned(),
                    content: "# Hi".to_owned(),
                }],
            }],
        };
        let built = doc_config(input);
        assert_eq!(built.docs[0].title, "G");
        assert_eq!(built.docs[0].pages[0].markdown, "# Hi");
        assert_eq!(built.docs[0].pages[0].path, "docs/intro.md");
    }

    #[test]
    fn check_reports_an_error_with_line_col() {
        let diags = enrich(
            &check_source(
                "//! m\npublic system S;\npublic container C for S {\n  f(): number { return ghost }\n}",
            ),
            "ignored",
        );
        assert!(diags.iter().any(|d| d.severity == "error"));
        assert!(diags.iter().any(|d| d.message.contains("ghost")));
    }

    #[test]
    fn references_span_modules_and_flag_the_declaration() {
        let offset = u32::try_from(SYS_SRC.find("Web").expect("Web in sys")).unwrap();
        let refs = references_at(&workspace(), &[], "sys", offset).expect("references");
        assert!(refs.occurrences.iter().any(|o| o.fqn == "sys" && o.decl));
        assert!(refs.occurrences.iter().any(|o| o.fqn == "ctx"));
    }

    #[test]
    fn references_on_blank_offset_is_none() {
        assert!(references_at(&workspace(), &[], "sys", 0).is_none());
    }

    #[test]
    fn check_clean_model_is_empty() {
        assert!(enrich(&check_source("//! m\npublic system S;"), "x").is_empty());
    }

    #[test]
    fn format_canonicalises() {
        let out = format_source("//! m\npublic   system    S;").expect("clean source formats");
        assert!(out.contains("system S"), "{out}");
    }

    #[test]
    fn emit_context_scene_is_json() {
        let json = emit_scene_of(
            &mods(&[("m", "//! m\npublic person P;\npublic system S;")]),
            "context",
            "",
        )
        .expect("context view projects");
        assert!(json.contains("context"));
    }

    #[test]
    fn emit_unknown_view_errors() {
        assert!(emit_scene_of(&mods(&[("m", "//! m\npublic system S;")]), "nope", "").is_err());
    }

    #[test]
    fn outline_lists_nodes_by_kind_and_triggers() {
        let nodes = outline_of(&mods(&[(
            "m",
            "//! m\npublic person P;\npublic system S;\npublic container C for S {\n  #[manual]\n  public Go(): void {}\n}",
        )]));
        assert!(nodes.iter().any(|n| n.kind == "person"));
        assert!(nodes.iter().any(|n| n.kind == "system"));
        assert!(nodes.iter().any(|n| n.kind == "container"));
        assert!(nodes.iter().any(|n| n.triggered));
    }

    #[test]
    fn outline_lists_features_under_their_target() {
        let nodes = outline_of(&mods(&[(
            "m",
            "//! m\npublic system S;\nfeature F for S {\n  given \"a\"\n  when \"b\"\n  then \"c\"\n}",
        )]));
        let feat = nodes.iter().find(|n| n.kind == "feature").expect("feature");
        assert_eq!(feat.fqn, "m::F");
        assert_eq!(feat.parent.as_deref(), Some("m::S"));
    }

    #[test]
    fn emit_scene_spans_modules() {
        let json = emit_scene_of(
            &mods(&[
                ("sys", "//! sys\npublic system S;\npublic container C for S;"),
                (
                    "comp",
                    "//! comp\ncomponent A for sys::C {\n  public Run(): void { B.Go() }\n}\ncomponent B for sys::C {\n  public Go(): void;\n}",
                ),
            ]),
            "component",
            "sys::C",
        )
        .expect("projects");
        assert!(
            json.contains("comp::A") && json.contains("comp::B"),
            "{json}"
        );
    }

    #[test]
    fn hover_on_a_node_reference_returns_markup() {
        let offset = u32::try_from(CTX_SRC.find("Web").expect("Web present") + 1).unwrap();
        let hover = hover_at(&workspace(), &[], "ctx", offset).expect("hovers");
        assert!(
            hover.contents.value.contains("container `sys::Web`"),
            "{hover:?}"
        );
    }

    #[test]
    fn hover_on_a_member_call_describes_the_callable() {
        let offset = u32::try_from(CTX_SRC.find("View").expect("View present") + 1).unwrap();
        let hover = hover_at(&workspace(), &[], "ctx", offset).expect("hovers");
        assert!(hover.contents.value.contains("callable `Web.View`"));
    }

    #[test]
    fn hover_on_blank_space_is_none() {
        assert!(hover_at(&workspace(), &[], "ctx", 0).is_none());
    }

    #[test]
    fn completion_after_module_path_is_scoped() {
        let offset = u32::try_from(CTX_SRC.find("sys::").expect("sys::") + "sys::".len()).unwrap();
        let items = completion_at(&workspace(), &[], "ctx", offset);
        assert!(items.iter().any(|c| c.label == "Web"));
        assert!(items.iter().any(|c| c.label == "Shop"));
        // MODULE kind (9) in the LSP CompletionItemKind enum.
        assert!(items.iter().any(|c| c.kind == 9));
        // general scope must not leak into a `::` context
        assert!(!items.iter().any(|c| c.label == "person"));
    }

    #[test]
    fn symbol_scene_projects_a_container() {
        let json = symbol_scene_of(&workspace(), "sys::Web").expect("projects");
        assert!(json.contains("sys::Web"), "{json}");
    }

    #[test]
    fn symbol_scene_projects_a_black_box_callable_as_a_sequence() {
        let json = symbol_scene_of(
            &mods(&[(
                "sys",
                "//! sys\npublic system Maps {\n  public Eta(at: Point): Result<Eta, Err>;\n}",
            )]),
            "sys::Maps::Eta",
        )
        .expect("projects");
        assert!(json.contains(r#""view":"sequence""#), "{json}");
        assert!(json.contains(r#""kind":"call""#), "{json}");
    }

    #[test]
    fn definition_resolves_a_data_field_to_its_owner_qualified_fqn() {
        let src = "//! m\npublic data Conv { id: uuid }\n";
        let offset = u32::try_from(src.find("id:").expect("`id` field")).unwrap();
        let fqn = definition_at(&mods(&[("m", src)]), &[], "m", offset).expect("resolves");
        assert_eq!(fqn, "m::Conv::id");
    }

    #[test]
    fn definition_qualifies_by_the_workspace_module_fqn_not_the_header() {
        let src = "//! header-name\npublic data Thing { id: uuid }\npublic system S;\npublic container C for S {\n  run(t: Thing): void {}\n}";
        let offset = u32::try_from(src.find("t: Thing").expect("param") + "t: ".len()).unwrap();
        let fqn = definition_at(&mods(&[("realmod", src)]), &[], "realmod", offset).expect("ok");
        assert_eq!(fqn, "realmod::Thing");
    }

    #[test]
    fn symbol_scene_projects_a_feature_flow() {
        let modules = mods(&[(
            "m",
            "//! m\npublic system S;\nfeature F for S {\n  given \"a\"\n  when \"b\"\n  then \"c\"\n}",
        )]);
        // A `feature` is not a graph node, but selecting it projects its flow
        // view — it must not error and crash the canvas.
        let json = symbol_scene_of(&modules, "m::F").expect("feature projects a flow scene");
        assert!(
            json.contains("\"view\":\"feature\""),
            "feature scene: {json}"
        );
        assert!(json.contains("\"target_fqn\":\"m::S\""));
        // A genuinely unknown symbol still errors.
        assert!(symbol_scene_of(&modules, "m::Nope").is_err());
    }

    #[test]
    fn symbol_scene_projects_a_data_entity_view() {
        let modules = mods(&[(
            "m",
            "//! m\npublic data Money { minor: number }\npublic data Order { total: Money }",
        )]);
        let json = symbol_scene_of(&modules, "m::Order").expect("data projects an entity scene");
        assert!(json.contains("\"view\":\"data\""), "data scene: {json}");
        assert!(
            json.contains("m::Money"),
            "referenced type pulled in: {json}"
        );
    }

    #[test]
    fn layout_scene_lays_out_data_and_feature_scenes() {
        let modules = mods(&[(
            "m",
            "//! m\npublic system S;\npublic data Order { id: string }\n\
             feature F for S {\n  given \"a\"\n  then \"b\"\n}",
        )]);
        for fqn in ["m::Order", "m::F"] {
            let scene = symbol_scene_of(&modules, fqn).expect("projects");
            let laid = layout_of(&scene).expect("lays out");
            assert!(laid.contains("\"width\":"), "positioned: {laid}");
        }
    }

    #[test]
    fn layout_scene_accepts_the_single_lifeline_fallback_shape() {
        let fallback = r#"{
            "view":"sequence",
            "entry":"m::F",
            "participants":[{"fqn":"m::F","kind":"callable"}],
            "items":[]
        }"#;
        let out = layout_of(fallback).expect("fallback scene lays out");
        assert!(out.contains("m::F"), "{out}");
    }

    #[test]
    fn layout_scene_lays_out_a_c4_scene() {
        let scene = emit_scene_of(
            &mods(&[("m", "//! m\npublic person P;\npublic system S;")]),
            "context",
            "",
        )
        .expect("context projects");
        let out = layout_of(&scene).expect("c4 scene lays out");
        assert!(out.contains(r#""nodes""#), "{out}");
        assert!(out.contains("m::P") && out.contains("m::S"), "{out}");
    }

    #[test]
    fn layout_scene_rejects_a_scene_missing_the_view_tag() {
        let untagged = r#"{"participants":[{"fqn":"m::F","kind":"callable"}],"items":[]}"#;
        assert!(layout_of(untagged).is_err());
    }

    #[test]
    fn check_workspace_attributes_to_referrer() {
        let results = check_workspace(
            &mods(&[
                ("a", "//! a\nsystem Hidden;"),
                ("b", "//! b\npublic container C for a::Hidden;"),
            ]),
            &[],
        );
        let b = results.iter().find(|r| r.fqn == "b").expect("module b");
        assert!(!b.diagnostics.is_empty(), "referrer carries the error");
    }

    #[test]
    fn dependency_modules_prefixes_vendored_by_lock_name() {
        let lock = "version = 1\n\n[[root]]\nname = \"banking\"\nsource = \"https://x/acme/banking\"\nrev = \"0123456789abcdef\"\npath = \"\"\n";
        let session = IdeSession::default();
        let out = session
            .dependency_modules(
                lock,
                vec![VendoredInput {
                    slug: "banking-0123456789ab".to_owned(),
                    fqn: "core".to_owned(),
                    source: "//! c\npublic system Ledger;\n".to_owned(),
                }],
                vec![],
            )
            .expect("resolves");
        assert!(out.iter().any(|m| m.fqn == "banking::core"));
    }

    #[test]
    fn completion_offers_external_dependency_module() {
        let local = mods(&[("m", "//! m\n\n")]);
        let external = mods(&[(
            "banking::core",
            "//! banking::core\n\npublic system Ledger;\n",
        )]);
        let offset = u32::try_from("//! m\n\n".len()).unwrap();
        let items = completion_at(&local, &external, "m", offset);
        assert!(
            items.iter().any(|c| c.label == "banking::core"),
            "{items:?}"
        );
    }

    #[test]
    fn set_source_edit_is_reflected_in_the_next_query() {
        let mut session = IdeSession::default();
        session.mount(
            vec![Module {
                fqn: "m".to_owned(),
                source: "//! m\npublic system S;\n".to_owned(),
            }],
            vec![],
        );
        assert!(
            session
                .diagnostics()
                .iter()
                .all(|r| r.diagnostics.is_empty())
        );
        // Introduce an undefined-identifier error; the next check sees it.
        session.set_source(
            "m",
            "//! m\npublic system S;\npublic container C for S {\n  f(): number { return ghost }\n}\n",
        );
        assert!(
            session
                .diagnostics()
                .iter()
                .any(|r| r.diagnostics.iter().any(|d| d.severity == "error"))
        );
    }
}
