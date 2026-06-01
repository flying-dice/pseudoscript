//! WebAssembly entry point: the `PseudoScript` compiler API for JavaScript
//! hosts (browser, Bun, Node, Deno).
//!
//! This crate is a thin, host-agnostic façade over the language core —
//! [`pseudoscript_syntax`] (parse), [`pseudoscript_format`] (format),
//! [`pseudoscript_model`] (the static checker), and [`pseudoscript_emit`]
//! (diagrams). It carries no engine and no transport: a tool that extends the
//! language (a Bun plugin, an online IDE, a CI check) compiles this to
//! `wasm32-unknown-unknown` with `wasm-bindgen` and calls the functions below.
//!
//! Every function is **JSON in / JSON out** (or plain strings) so the contract
//! is identical across hosts and raw-wasm callers. Spans are enriched with
//! 1-based line/column for editor use, alongside the raw byte offsets.
//!
//! # Functions
//!
//! - [`parse`] — syntax diagnostics only (fast path for an editor's squiggles).
//! - [`check`] / [`check_modules`] — full parse + static analysis, single file
//!   or a multi-module workspace.
//! - [`format`] — canonical formatting.
//! - [`emit_scene`] / [`emit_svg`] — a diagram view as scene JSON or SVG.
//!
//! Each `#[wasm_bindgen]` function is a thin wrapper over a `*_impl` that
//! returns `Result<_, String>`; the wrapper maps the error to a `JsError`. The
//! `*_impl`s carry the logic and are unit-tested on the host (a `JsError`
//! cannot be constructed off-wasm).
//!
//! The language-intelligence functions — [`hover`], [`completion`],
//! [`semantic_tokens`], [`folding_ranges`] — are an **LSP-over-wasm** bridge:
//! they call [`pseudoscript_lsp_core`] (the same handlers the stdio server uses)
//! and serialise its `lsp_types` results to JSON, so the WASM API is byte-for-
//! byte the LSP API minus the `tower-lsp`/`tokio` transport. The diagram and doc
//! exports ([`emit_scene`], [`symbol_scene`], …) have no LSP equivalent and are
//! WASM-only.

use std::collections::HashMap;

use pseudoscript_doc::{
    DocConfig, DocGroup, DocPage, RenderError, RenderedPage, SsrEngine, Theme, ssr_bundle,
    try_render_site_with,
};
use pseudoscript_emit::{
    Scene, View, graph_of_source, layout_sequence_scene, project, project_symbol, render_svg,
};
use pseudoscript_format::format as format_source;
use pseudoscript_model::{
    Graph, NodeKind, Workspace, WorkspaceModule, check as check_source, check_workspace_modules,
    graph as build_graph, resolve::resolve_at,
};
// The shared LSP API: completion, hover, semantic tokens, and folding are served
// here exactly as the stdio server serves them — same `lsp_types` results.
use pseudoscript_lsp_core::{analysis, complete, convert, semantic, symbols};
use pseudoscript_syntax::{
    Diagnostic, LineIndex, Severity, TokenKind, parse as parse_source, tokenize,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Routes Rust panics to the browser console with a readable stack. Runs once
/// on module instantiation (wasm only).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// The crate version, for host-side compatibility checks.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Parses `source` and returns its **syntax** diagnostics as a JSON array.
/// Faster than [`check`] — no static analysis — for an editor's parse-error
/// squiggles on every keystroke.
#[wasm_bindgen]
#[must_use]
pub fn parse(source: &str) -> String {
    let parsed = parse_source(source);
    to_json(&enrich(&parsed.diagnostics, source))
}

/// Parses and statically checks `source` as a single module, returning every
/// diagnostic (parse errors then static errors) as a JSON array.
#[wasm_bindgen]
#[must_use]
pub fn check(source: &str) -> String {
    to_json(&enrich(&check_source(source), source))
}

/// Checks a multi-module workspace. `modules_json` is a JSON array of
/// `{ "fqn": string, "source": string }`. Returns a JSON array of
/// `{ "fqn": string, "diagnostics": Diagnostic[] }`, with each module's
/// diagnostics attributed to it (cross-module errors land on the referring
/// module, §8.2).
///
/// # Errors
///
/// Returns an error when `modules_json` is not valid JSON of the expected
/// shape.
#[wasm_bindgen]
pub fn check_modules(modules_json: &str) -> Result<String, JsError> {
    check_modules_impl(modules_json).map_err(|e| JsError::new(&e))
}

/// Formats `source` into its canonical form.
///
/// # Errors
///
/// Returns an error when `source` does not parse (formatting requires a valid
/// parse tree).
#[wasm_bindgen]
pub fn format(source: &str) -> Result<String, JsError> {
    format_impl(source).map_err(|e| JsError::new(&e))
}

/// Projects a diagram view from `source` and returns the laid-out [`Scene`] as
/// JSON. `view` is one of `context`, `container`, `component`, or `sequence`;
/// `target` is the boundary FQN (container/component) or entry callable FQN
/// (sequence), and is ignored for `context`.
///
/// # Errors
///
/// Returns an error for an unknown `view`, or when the view cannot be projected
/// (the target resolves to no node, or the wrong kind).
#[wasm_bindgen]
pub fn emit_scene(source: &str, view: &str, target: &str) -> Result<String, JsError> {
    project_view(source, view, target)
        .map(|scene| to_json(&scene))
        .map_err(|e| JsError::new(&e))
}

/// Projects a diagram view from `source` and renders it to a self-contained SVG
/// string. See [`emit_scene`] for the `view`/`target` arguments.
///
/// # Errors
///
/// Returns an error for an unknown `view`, or when the view cannot be projected.
#[wasm_bindgen]
pub fn emit_svg(source: &str, view: &str, target: &str) -> Result<String, JsError> {
    project_view(source, view, target)
        .map(|scene| render_svg(&scene))
        .map_err(|e| JsError::new(&e))
}

/// AST-aware semantic tokens for `source`, as the JSON of an LSP
/// `SemanticTokens` (the delta-encoded `data` array over UTF-16 positions; the
/// `token_type` field indexes the [`pseudoscript_lsp_core::semantic`] legend).
/// Identical to the stdio server's `textDocument/semanticTokens/full` response —
/// the editor decodes and decorates it, replacing any hand-written tokenizer.
#[wasm_bindgen]
#[must_use]
pub fn semantic_tokens(source: &str) -> String {
    to_json(&semantic::semantic_tokens(source))
}

/// Foldable regions of `source` as the JSON of an LSP `FoldingRange` array
/// (`{ startLine, endLine, kind }`, 0-based lines) — every multi-line
/// declaration and statement block. Identical to the stdio server's
/// `textDocument/foldingRange` response; the editor folds these instead of
/// brace-matching in JS.
#[wasm_bindgen]
#[must_use]
pub fn folding_ranges(source: &str) -> String {
    to_json(&symbols::folding_ranges(source))
}

/// Lists the nodes declared in `source` as a JSON array of
/// `{ fqn, name, kind, triggered }`. A host uses this to populate a diagram's
/// target picker: `container` views target a `system`, `component` views a
/// `container`, and `sequence` views a `triggered` callable.
#[wasm_bindgen]
#[must_use]
pub fn outline(source: &str) -> String {
    let index = LineIndex::new(source);
    to_json(&outline_nodes(
        &graph_of_source(source),
        |_module, offset| index.line_col(offset),
    ))
}

/// Like [`outline`], but over a whole workspace (`modules_json` is the same
/// `[{fqn, source}]` shape as [`check_modules`]), so a cross-module container or
/// system is a valid diagram target.
///
/// # Errors
///
/// Returns an error when `modules_json` is not valid JSON of the expected shape.
#[wasm_bindgen]
pub fn outline_modules(modules_json: &str) -> Result<String, JsError> {
    let modules = modules_from_json(modules_json).map_err(|e| JsError::new(&e))?;
    let indices: HashMap<&str, LineIndex> = modules
        .iter()
        .map(|m| (m.fqn.as_str(), LineIndex::new(&m.source)))
        .collect();
    let graph = build_graph(&modules);
    Ok(to_json(&outline_nodes(&graph, |module, offset| {
        indices
            .get(module)
            .map_or((1, 1), |index| index.line_col(offset))
    })))
}

/// Projects a diagram view over a whole workspace graph, so it shows nodes and
/// edges across modules (a container's components, cross-system calls). Same
/// `view`/`target` arguments as [`emit_scene`]; `modules_json` is `[{fqn,
/// source}]`.
///
/// # Errors
///
/// Returns an error for invalid JSON, an unknown `view`, or a view that cannot
/// be projected.
#[wasm_bindgen]
pub fn emit_scene_modules(modules_json: &str, view: &str, target: &str) -> Result<String, JsError> {
    emit_scene_modules_impl(modules_json, view, target).map_err(|e| JsError::new(&e))
}

/// Resolves the symbol under `offset` (a byte offset) in module `module_fqn` and
/// returns it as an LSP `Hover` (`{ contents: { kind, value }, range }`,
/// Markdown), or `null` when the cursor rests on no resolvable symbol. Served by
/// the shared [`pseudoscript_lsp_core::analysis::hover`] — identical to the
/// stdio server's `textDocument/hover`, no diagram. The interactive diagram is a
/// separate concern: [`symbol_scene`] / [`symbol_svg`]. `modules_json` is the
/// `[{fqn, source}]` workspace shape.
///
/// # Errors
///
/// Returns an error when `modules_json` is not valid JSON of the expected shape.
#[wasm_bindgen]
pub fn hover(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, JsError> {
    hover_impl(modules_json, module_fqn, offset).map_err(|e| JsError::new(&e))
}

/// Resolves the symbol under `offset` (a byte offset) in module `module_fqn` to
/// the FQN of its declaration, for go-to-definition. Returns the FQN as a JSON
/// string, or `null` when the cursor rests on no resolvable symbol. Unlike
/// [`hover`] it renders no diagram, so it is cheap enough for a click handler.
/// `modules_json` is the `[{fqn, source}]` workspace shape.
///
/// # Errors
///
/// Returns an error when `modules_json` is not valid JSON of the expected shape.
#[wasm_bindgen]
pub fn definition(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, JsError> {
    definition_impl(modules_json, module_fqn, offset).map_err(|e| JsError::new(&e))
}

/// Finds every occurrence of the symbol under `offset` in module `module_fqn`
/// across the whole workspace — find-usages. Returns JSON
/// `{ fqn, title, occurrences: [{ fqn, line, col, end_line, end_col, text, decl }] }`,
/// where each occurrence carries its 1-based position, the trimmed source line
/// for a preview, and `decl` marking the declaration site. Returns `null` when
/// the cursor rests on no resolvable symbol. `modules_json` is `[{fqn, source}]`.
///
/// # Errors
///
/// Returns an error when `modules_json` is not valid JSON of the expected shape.
#[wasm_bindgen]
pub fn references(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, JsError> {
    references_impl(modules_json, module_fqn, offset).map_err(|e| JsError::new(&e))
}

/// Context-aware completion at `offset` (a byte offset) in module `module_fqn`,
/// as a JSON array of LSP `CompletionItem`s (`{label, kind, detail}`, where
/// `kind` is the integer `CompletionItemKind`). Scoped to the trigger before the
/// caret (`.`/`::`/`#[`/type-position/general); the client filters against the
/// typed prefix. Served by the shared [`pseudoscript_lsp_core::complete`] —
/// identical to the stdio server's `textDocument/completion`. `modules_json` is
/// the `[{fqn, source}]` workspace shape.
///
/// # Errors
///
/// Returns an error when `modules_json` is not valid JSON of the expected shape.
#[wasm_bindgen]
pub fn completion(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, JsError> {
    completion_impl(modules_json, module_fqn, offset).map_err(|e| JsError::new(&e))
}

/// Projects the fitting diagram for the symbol `fqn` over the whole workspace
/// and returns its laid-out [`Scene`] as JSON (the interactive counterpart of
/// [`hover`]'s `svg`, for a side panel or full-screen view). See
/// [`project_symbol`] for how the view is chosen.
///
/// # Errors
///
/// Returns an error for invalid JSON, an unknown symbol, or a symbol that
/// cannot be projected.
#[wasm_bindgen]
pub fn symbol_scene(modules_json: &str, fqn: &str) -> Result<String, JsError> {
    symbol_scene_impl(modules_json, fqn).map_err(|e| JsError::new(&e))
}

/// Renders the fitting diagram for the symbol `fqn` (see [`project_symbol`]) to
/// a self-contained SVG string over the whole workspace — the live, re-derivable
/// form of [`hover`]'s `svg` for a docked side panel. `modules_json` is `[{fqn,
/// source}]`.
///
/// # Errors
///
/// Returns an error for invalid JSON, an unknown symbol, or a symbol that
/// cannot be projected.
#[wasm_bindgen]
pub fn symbol_svg(modules_json: &str, fqn: &str) -> Result<String, JsError> {
    symbol_svg_impl(modules_json, fqn).map_err(|e| JsError::new(&e))
}

/// Positions a sequence [`Scene`] (as JSON) into absolute coordinates, returning
/// the layout as JSON. The host collapses the scene to a chosen depth first,
/// then hands it here; the layout engine owns all geometry. A non-sequence scene
/// is an error.
///
/// # Errors
///
/// Returns an error for invalid JSON or a non-sequence scene.
#[wasm_bindgen]
pub fn layout_scene(scene_json: &str) -> Result<String, JsError> {
    layout_scene_impl(scene_json).map_err(|e| JsError::new(&e))
}

/// The Svelte SSR bundle (`ssr.js`) the host evaluates in its own JavaScript
/// engine — the browser — to define `globalThis.SSR.renderPage`. Hand that
/// function back to [`render_doc_site`] as the `render` callback.
#[wasm_bindgen]
#[must_use]
pub fn doc_ssr_bundle() -> String {
    ssr_bundle().to_owned()
}

/// Renders the whole documentation site for a workspace, exactly as the CLI's
/// `pds doc` does, driving server-side rendering through the host's JavaScript
/// engine rather than an embedded one.
///
/// `render` is a JS function `(propsJson: string) => string` returning one
/// page's `{head, body}` JSON — typically `SSR.renderPage` from the evaluated
/// [`doc_ssr_bundle`]. `config_json` is `{ name, theme?, logo? }`. Returns the
/// site as JSON `[{ path, contents }]` for the host to write.
///
/// # Errors
///
/// Returns an error for invalid `modules_json`/`config_json`, or when a page
/// fails to render (a bundle/engine defect — not user model data).
#[wasm_bindgen]
pub fn render_doc_site(
    modules_json: &str,
    config_json: &str,
    render: &js_sys::Function,
) -> Result<String, JsError> {
    let modules = modules_from_json(modules_json).map_err(|e| JsError::new(&e))?;
    let config = doc_config(config_json).map_err(|e| JsError::new(&e))?;
    let engine = HostEngine {
        render: render.clone(),
    };
    let site = try_render_site_with(&build_graph(&modules), &config, &engine)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let files: Vec<SiteFileOut> = site
        .files
        .iter()
        .map(|f| SiteFileOut {
            path: f.path.clone(),
            contents: f.contents.clone(),
        })
        .collect();
    Ok(to_json(&files))
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

/// Builds a [`DocConfig`] from the host's `{ name, theme?, logo?, docs? }` JSON.
/// `docs` groups carry each page's already-loaded Markdown `content` — the host
/// reads the files the manifest names; this crate (and the CLI) only render.
fn doc_config(config_json: &str) -> Result<DocConfig, String> {
    let input: DocConfigInput = serde_json::from_str(config_json).map_err(|e| e.to_string())?;
    Ok(DocConfig {
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
    })
}

/// Parses a `pds.toml` string into the doc manifest the host needs to build the
/// sidebar and read its pages: JSON
/// `{ name?, theme?, logo?, lang?, sidebar: [{ title, items: [{ title, path }] }] }`.
/// The host loads each `path`, then hands the assembled config (with page
/// `content`) back to [`render_doc_site`]. Uses the same `toml` parser as the
/// native CLI, so the two agree on the schema.
///
/// # Errors
///
/// Returns an error when `toml` is not valid TOML of the `[doc]` shape.
#[wasm_bindgen]
pub fn doc_manifest(toml: &str) -> Result<String, JsError> {
    doc_manifest_impl(toml).map_err(|e| JsError::new(&e))
}

fn doc_manifest_impl(toml_src: &str) -> Result<String, String> {
    let manifest: ManifestInput = toml::from_str(toml_src).map_err(|e| e.to_string())?;
    Ok(to_json(&manifest.doc))
}

// ---- logic (host-testable; no `JsError`, which cannot exist off-wasm) ------

/// Projects a graph's nodes to outline entries. `line_col` maps a node's owning
/// module and byte offset to a 1-based position — single-source callers ignore
/// the module; workspace callers index per module.
fn outline_nodes(
    graph: &Graph,
    mut line_col: impl FnMut(&str, u32) -> (u32, u32),
) -> Vec<OutlineNode> {
    graph
        .nodes()
        .iter()
        .map(|n| {
            let (line, col) = line_col(&n.module, n.span.start);
            OutlineNode {
                fqn: n.fqn.clone(),
                name: n.name.clone(),
                kind: n.kind,
                triggered: !n.triggers.is_empty(),
                line,
                col,
                parent: n.parent.clone(),
            }
        })
        .collect()
}

/// Parses the `[{fqn, source}]` workspace JSON into modules.
fn modules_from_json(modules_json: &str) -> Result<Vec<WorkspaceModule>, String> {
    let inputs: Vec<InputModule> = serde_json::from_str(modules_json).map_err(|e| e.to_string())?;
    Ok(inputs
        .into_iter()
        .map(|m| WorkspaceModule::new(m.fqn, m.source))
        .collect())
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
        other => Err(format!("unknown view `{other}`")),
    }
}

fn emit_scene_modules_impl(modules_json: &str, view: &str, target: &str) -> Result<String, String> {
    let graph = build_graph(&modules_from_json(modules_json)?);
    let scene = project(&graph, view_of(view, target)?).map_err(|e| e.to_string())?;
    Ok(to_json(&scene))
}

fn hover_impl(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, String> {
    let modules = modules_from_json(modules_json)?;
    let src = modules
        .iter()
        .find(|m| m.fqn == module_fqn)
        .map_or("", |m| m.source.as_str());
    let workspace = Workspace::build(
        modules
            .iter()
            .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
    );
    let index = LineIndex::new(src);
    let position = convert::offset_to_position(src, &index, offset);
    Ok(to_json(&analysis::hover(
        &workspace, module_fqn, src, position,
    )))
}

fn definition_impl(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, String> {
    let modules = modules_from_json(modules_json)?;
    let src = modules
        .iter()
        .find(|m| m.fqn == module_fqn)
        .map_or("", |m| m.source.as_str());
    let workspace = Workspace::build(
        modules
            .iter()
            .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
    );
    Ok(match resolve_at(&workspace, module_fqn, src, offset) {
        Some(hit) => to_json(&hit.target_fqn),
        None => "null".to_owned(),
    })
}

fn references_impl(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, String> {
    let modules = modules_from_json(modules_json)?;
    let src = modules
        .iter()
        .find(|m| m.fqn == module_fqn)
        .map_or("", |m| m.source.as_str());
    let workspace = Workspace::build(
        modules
            .iter()
            .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
    );
    let Some(target) = resolve_at(&workspace, module_fqn, src, offset) else {
        return Ok("null".to_owned()); // cursor on no resolvable symbol
    };

    // Scan every name-position identifier in the workspace, keeping those that
    // resolve to the same definition (mirrors pseudoscript-lsp's refs engine; a
    // `::` qualifier names a module, never the symbol).
    let mut occurrences = Vec::new();
    for module in &modules {
        let index = LineIndex::new(&module.source);
        let lines: Vec<&str> = module.source.lines().collect();
        let tokens = tokenize(&module.source);
        for (i, token) in tokens.iter().enumerate() {
            if token.kind != TokenKind::Ident
                || tokens
                    .get(i + 1)
                    .is_some_and(|t| t.kind == TokenKind::ColonColon)
            {
                continue;
            }
            let Some(hit) = resolve_at(&workspace, &module.fqn, &module.source, token.span.start)
            else {
                continue;
            };
            if hit.target_module != target.target_module || hit.target_span != target.target_span {
                continue;
            }
            let (line, col) = index.line_col(token.span.start);
            let (end_line, end_col) = index.line_col(token.span.end);
            // Trim the preview line for display, then map the token's byte span
            // (a 1-based byte column) into char offsets within the trimmed text,
            // so a host can highlight the symbol inside the preview.
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
            occurrences.push(RefOccurrence {
                fqn: module.fqn.clone(),
                line,
                col,
                end_line,
                end_col,
                text,
                match_start,
                match_end,
                decl: module.fqn == target.target_module && token.span == target.target_span,
            });
        }
    }

    Ok(to_json(&ReferencesResult {
        fqn: target.target_fqn,
        title: target.title,
        occurrences,
    }))
}

fn completion_impl(modules_json: &str, module_fqn: &str, offset: u32) -> Result<String, String> {
    let modules = modules_from_json(modules_json)?;
    let src = modules
        .iter()
        .find(|m| m.fqn == module_fqn)
        .map_or("", |m| m.source.as_str());
    let workspace = Workspace::build(
        modules
            .iter()
            .map(|m| (m.fqn.clone(), parse_source(&m.source).ast)),
    );
    let index = LineIndex::new(src);
    let position = convert::offset_to_position(src, &index, offset);
    Ok(to_json(&complete::completion(
        &workspace, module_fqn, src, position,
    )))
}

fn symbol_scene_impl(modules_json: &str, fqn: &str) -> Result<String, String> {
    let graph = build_graph(&modules_from_json(modules_json)?);
    let scene = project_symbol(&graph, fqn).map_err(|e| e.to_string())?;
    Ok(to_json(&scene))
}

fn symbol_svg_impl(modules_json: &str, fqn: &str) -> Result<String, String> {
    let graph = build_graph(&modules_from_json(modules_json)?);
    let scene = project_symbol(&graph, fqn).map_err(|e| e.to_string())?;
    Ok(render_svg(&scene))
}

fn layout_scene_impl(scene_json: &str) -> Result<String, String> {
    let scene: Scene = serde_json::from_str(scene_json).map_err(|e| e.to_string())?;
    match scene {
        Scene::Sequence(seq) => Ok(to_json(&layout_sequence_scene(&seq))),
        Scene::C4(_) => Err("layout_scene expects a sequence scene".to_owned()),
    }
}

fn check_modules_impl(modules_json: &str) -> Result<String, String> {
    let inputs: Vec<InputModule> = serde_json::from_str(modules_json).map_err(|e| e.to_string())?;
    let modules: Vec<WorkspaceModule> = inputs
        .iter()
        .map(|m| WorkspaceModule::new(m.fqn.clone(), m.source.clone()))
        .collect();

    let results: Vec<ModuleResult> = check_workspace_modules(&modules)
        .into_iter()
        .map(|module| {
            // Enrich each span against the source it indexes into.
            let source = inputs
                .iter()
                .find(|m| m.fqn == module.fqn)
                .map_or("", |m| m.source.as_str());
            ModuleResult {
                fqn: module.fqn,
                diagnostics: enrich(&module.diagnostics, source),
            }
        })
        .collect();
    Ok(to_json(&results))
}

fn format_impl(source: &str) -> Result<String, String> {
    format_source(source).map_err(|e| e.to_string())
}

/// Builds the graph for a single source and projects `view`/`target`.
fn project_view(source: &str, view: &str, target: &str) -> Result<Scene, String> {
    project(&graph_of_source(source), view_of(view, target)?).map_err(|e| e.to_string())
}

// ---- DTOs ------------------------------------------------------------------

/// One input module for [`check_modules`].
#[derive(Serialize, Deserialize)]
struct InputModule {
    fqn: String,
    source: String,
}

/// The host's documentation config for [`render_doc_site`]: site name, optional
/// theme word (`dark`/`light`, default `dark`), optional logo path, and the
/// authored doc groups with their pages' already-loaded Markdown `content`.
#[derive(Deserialize)]
struct DocConfigInput {
    name: String,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    docs: Vec<DocGroupInput>,
}

/// One doc group in [`DocConfigInput`]: a heading and its pages (with content).
#[derive(Deserialize)]
struct DocGroupInput {
    #[serde(default)]
    title: String,
    #[serde(default)]
    items: Vec<DocItemInput>,
}

/// One page in a [`DocGroupInput`]: its title, source path, and Markdown body.
#[derive(Deserialize)]
struct DocItemInput {
    #[serde(default)]
    title: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    content: String,
}

/// A `pds.toml` for [`doc_manifest`]: only its `[doc]` table is read.
#[derive(Deserialize)]
struct ManifestInput {
    #[serde(default)]
    doc: DocManifest,
}

/// The `[doc]` table parsed for the host (no page content — the host loads it).
/// Serialised straight back out as the manifest JSON.
#[derive(Default, Deserialize, Serialize)]
struct DocManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<String>,
    #[serde(default)]
    sidebar: Vec<DocManifestGroup>,
}

/// One `[[doc.sidebar]]` group in the host-facing manifest.
#[derive(Default, Deserialize, Serialize)]
struct DocManifestGroup {
    #[serde(default)]
    title: String,
    #[serde(default)]
    items: Vec<DocManifestItem>,
}

/// One `{ title, path }` page entry in the host-facing manifest.
#[derive(Default, Deserialize, Serialize)]
struct DocManifestItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    path: String,
}

/// One rendered site file returned by [`render_doc_site`].
#[derive(Serialize)]
struct SiteFileOut {
    path: String,
    contents: String,
}

/// One module's diagnostics in the [`check_modules`] result.
#[derive(Serialize)]
struct ModuleResult {
    fqn: String,
    diagnostics: Vec<WasmDiagnostic>,
}

/// One declared node, for the [`outline`] target picker. `kind` serialises
/// lowercase (`person`/`system`/`container`/`component`/`data`/`callable`);
/// `triggered` marks a callable that carries a trigger macro (a sequence entry).
/// `line`/`col` are the 1-based position of the node's name in its own module,
/// for an editor to jump to the declaration. `parent` is the FQN of the
/// enclosing node (the `for`/owning parent, §6) — the C4 containment, not the
/// `::` path — or `null` for a top-level person/system/data.
#[derive(Serialize)]
struct OutlineNode {
    fqn: String,
    name: String,
    kind: NodeKind,
    triggered: bool,
    line: u32,
    col: u32,
    parent: Option<String>,
}

/// The result of [`references`]: the resolved symbol's `fqn`/`title` plus every
/// occurrence across the workspace.
#[derive(Serialize)]
struct ReferencesResult {
    fqn: String,
    title: String,
    occurrences: Vec<RefOccurrence>,
}

/// One find-usages hit: its module `fqn`, 1-based span, the trimmed source line
/// for a preview, and `decl` marking the declaration site. `match_start`/
/// `match_end` are char offsets into `text` bounding the symbol token, so a host
/// can highlight the occurrence within the line.
#[derive(Serialize)]
struct RefOccurrence {
    fqn: String,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
    text: String,
    match_start: u32,
    match_end: u32,
    decl: bool,
}

/// A diagnostic enriched with 1-based line/column for both span ends, in
/// addition to the raw byte offsets.
#[derive(Serialize)]
struct WasmDiagnostic {
    severity: &'static str,
    message: String,
    code: Option<String>,
    start: u32,
    end: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
}

/// Maps each diagnostic's byte span to line/column against `source`.
fn enrich(diagnostics: &[Diagnostic], source: &str) -> Vec<WasmDiagnostic> {
    let index = LineIndex::new(source);
    diagnostics
        .iter()
        .map(|d| {
            let (start_line, start_col) = index.line_col(d.span.start);
            let (end_line, end_col) = index.line_col(d.span.end);
            WasmDiagnostic {
                severity: severity_word(d.severity),
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

/// The lowercase wire word for a severity.
fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// Serialises a value to JSON. The DTOs above never fail to serialise; `null`
/// is the (unreached) valid-JSON fallback so this stays panic-free.
fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        check, check_modules_impl, completion_impl, doc_config, doc_manifest_impl,
        emit_scene_modules_impl, format_impl, hover_impl, outline, parse, project_view,
        references_impl, symbol_scene_impl, symbol_svg_impl, to_json,
    };

    #[test]
    fn doc_manifest_parses_doc_table_and_sidebar() {
        let toml = r#"
            [doc]
            name = "Banking"

            [[doc.sidebar]]
            title = "Start"
            items = [{ title = "Intro", path = "docs/intro.md" }]
        "#;
        let json = doc_manifest_impl(toml).expect("valid manifest");
        assert!(json.contains(r#""name":"Banking""#), "{json}");
        assert!(json.contains(r#""path":"docs/intro.md""#), "{json}");
        // Absent keys are omitted, not null.
        assert!(!json.contains("logo"), "{json}");
    }

    #[test]
    fn doc_config_maps_page_content() {
        let config = r##"{
            "name": "X",
            "docs": [{ "title": "G", "items": [
              { "title": "Intro", "path": "docs/intro.md", "content": "# Hi" }
            ]}]
        }"##;
        let built = doc_config(config).expect("valid config");
        assert_eq!(built.docs[0].title, "G");
        assert_eq!(built.docs[0].pages[0].markdown, "# Hi");
        assert_eq!(built.docs[0].pages[0].path, "docs/intro.md");
    }

    /// `ctx`'s source: a person calling a container in `sys`.
    const CTX_SRC: &str =
        "//! ctx\npublic person User {\n  public Use(): void { sys::Web.View() }\n}";
    /// `sys`'s source: a system with one container that has a bodied callable.
    const SYS_SRC: &str = "//! sys\npublic system Shop;\npublic container Web for Shop {\n  public View(): void {}\n}";

    /// The two modules as a `[{fqn, source}]` workspace JSON.
    fn workspace_json() -> String {
        to_json(&[
            super::InputModule {
                fqn: "ctx".to_owned(),
                source: CTX_SRC.to_owned(),
            },
            super::InputModule {
                fqn: "sys".to_owned(),
                source: SYS_SRC.to_owned(),
            },
        ])
    }

    #[test]
    fn check_reports_an_error_with_line_col() {
        let json = check(
            "//! m\npublic system S;\npublic container C for S {\n  f(): number { return ghost }\n}",
        );
        assert!(json.contains(r#""severity":"error""#), "{json}");
        assert!(json.contains("ghost"), "{json}");
        assert!(json.contains(r#""start_line""#), "{json}");
    }

    #[test]
    fn references_span_modules_and_flag_the_declaration() {
        // `Web` is declared in `sys` and called (`sys::Web.View()`) from `ctx`.
        let offset = SYS_SRC.find("Web").expect("Web in sys") as u32;
        let json = references_impl(&workspace_json(), "sys", offset).expect("references");
        // declaration in sys + qualified use in ctx
        assert!(json.contains(r#""fqn":"sys""#), "{json}");
        assert!(json.contains(r#""fqn":"ctx""#), "{json}");
        assert!(json.contains(r#""decl":true"#), "{json}");
    }

    #[test]
    fn references_on_blank_offset_is_null() {
        assert_eq!(
            references_impl(&workspace_json(), "sys", 0).expect("ok"),
            "null"
        );
    }

    #[test]
    fn check_clean_model_is_empty_array() {
        assert_eq!(check("//! m\npublic system S;"), "[]");
    }

    #[test]
    fn parse_of_clean_source_has_no_syntax_errors() {
        assert_eq!(parse("//! m\npublic system S;"), "[]");
    }

    #[test]
    fn format_canonicalises() {
        let out = format_impl("//! m\npublic   system    S;").expect("clean source formats");
        assert!(out.contains("system S"), "{out}");
    }

    #[test]
    fn emit_context_scene_is_json() {
        let scene = project_view("//! m\npublic person P;\npublic system S;", "context", "")
            .expect("context view projects");
        assert!(super::to_json(&scene).contains("context"));
    }

    #[test]
    fn emit_unknown_view_errors() {
        assert!(project_view("//! m\npublic system S;", "nope", "").is_err());
    }

    #[test]
    fn outline_lists_nodes_by_kind_and_triggers() {
        let json = outline(
            "//! m\npublic person P;\npublic system S;\npublic container C for S {\n  #[manual]\n  public Go(): void {}\n}",
        );
        assert!(json.contains(r#""kind":"person""#), "{json}");
        assert!(json.contains(r#""kind":"system""#), "{json}");
        assert!(json.contains(r#""kind":"container""#), "{json}");
        // the triggered callable is flagged for the sequence picker
        assert!(json.contains(r#""triggered":true"#), "{json}");
    }

    #[test]
    fn emit_scene_modules_spans_modules() {
        // A container in one module, its components (with a call between them) in
        // another — the workspace component view shows both and their edge.
        let input = r#"[
            {"fqn":"sys","source":"//! sys\npublic system S;\npublic container C for S;"},
            {"fqn":"comp","source":"//! comp\ncomponent A for sys::C {\n  public Run(): void { B.Go() }\n}\ncomponent B for sys::C {\n  public Go(): void;\n}"}
        ]"#;
        let json = emit_scene_modules_impl(input, "component", "sys::C").expect("projects");
        assert!(json.contains("comp::A"), "{json}");
        assert!(json.contains("comp::B"), "{json}");
    }

    #[test]
    fn hover_on_a_node_reference_returns_lsp_hover() {
        // Cursor on `Web` in `sys::Web.View()` — resolves to the container. The
        // result is an `lsp_types::Hover` (markup), no diagram SVG.
        let offset = (CTX_SRC.find("Web").expect("Web present") + 1) as u32;
        let json = hover_impl(&workspace_json(), "ctx", offset).expect("hovers");
        assert!(json.contains("container `sys::Web`"), "{json}");
        assert!(json.contains(r#""contents""#), "{json}");
        assert!(
            !json.contains("<svg"),
            "hover must not carry a diagram: {json}"
        );
    }

    #[test]
    fn hover_on_a_member_call_describes_the_callable() {
        // Cursor on `View` in `sys::Web.View()` — resolves to the callable.
        let offset = (CTX_SRC.find("View").expect("View present") + 1) as u32;
        let json = hover_impl(&workspace_json(), "ctx", offset).expect("hovers");
        assert!(json.contains("callable `Web.View`"), "{json}");
    }

    #[test]
    fn hover_on_blank_space_is_null() {
        let json = hover_impl(&workspace_json(), "ctx", 0).expect("hovers");
        assert_eq!(json, "null");
    }

    #[test]
    fn completion_after_module_path_is_scoped() {
        // Caret right after `sys::` (before `Web`) — offers only module sys's
        // public symbols (lsp_types::CompletionItem), no general keyword leak.
        let offset = (CTX_SRC.find("sys::").expect("sys:: present") + "sys::".len()) as u32;
        let json = completion_impl(&workspace_json(), "ctx", offset).expect("completes");
        assert!(json.contains(r#""label":"Web""#), "{json}");
        assert!(json.contains(r#""label":"Shop""#), "{json}");
        // MODULE kind (9) in the LSP CompletionItemKind enum
        assert!(json.contains(r#""kind":9"#), "{json}");
        // general scope must not leak into a `::` context
        assert!(
            !json.contains(r#""label":"person""#),
            "general scope leaked: {json}"
        );
    }

    #[test]
    fn symbol_scene_projects_a_container() {
        let json = symbol_scene_impl(&workspace_json(), "sys::Web").expect("projects");
        assert!(json.contains("sys::Web"), "{json}");
    }

    #[test]
    fn symbol_scene_projects_a_black_box_callable_as_a_sequence() {
        // A black-box callable (signature, no body) projects a sequence with at
        // least one message, not its owner's structural C4 view (§9.2).
        let input = r#"[
            {"fqn":"sys","source":"//! sys\npublic system Maps {\n  public Eta(at: Point): Result<Eta, Err>;\n}"}
        ]"#;
        let json = symbol_scene_impl(input, "sys::Maps::Eta").expect("projects");
        assert!(json.contains(r#""view":"sequence""#), "{json}");
        assert!(json.contains(r#""kind":"call""#), "{json}");
        assert!(json.contains("sys::Maps"), "{json}");
    }

    #[test]
    fn symbol_svg_renders_a_system_to_svg() {
        // A system's container view frames its containers — `Web` appears as a card.
        let svg = symbol_svg_impl(&workspace_json(), "sys::Shop").expect("renders");
        assert!(svg.contains("<svg"), "{svg}");
        assert!(svg.contains("Web"), "{svg}");
    }

    #[test]
    fn check_modules_attributes_to_referrer() {
        let input = r#"[
            {"fqn":"a","source":"//! a\nsystem Hidden;"},
            {"fqn":"b","source":"//! b\npublic container C for a::Hidden;"}
        ]"#;
        let out = check_modules_impl(input).expect("valid input checks");
        assert!(out.contains(r#""fqn":"b""#), "{out}");
    }
}
