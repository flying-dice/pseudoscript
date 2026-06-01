//! Static analysis for `PseudoScript` (`LANG.md` §3.5, §4, §5.1, §6, §8, §2.4).
//!
//! This crate sits above [`pseudoscript_syntax`]: it parses a `.pds` module,
//! resolves its declarations into a [`Model`] symbol table, then runs the
//! well-formedness checks, returning the shared [`Diagnostic`] type. It is
//! WASM-safe — no threads, filesystem, time, or native dependencies.
//!
//! # Surfaces
//!
//! - [`check`] — parse and analyze source text, returning the full diagnostic
//!   set (parse errors *and* static errors). The CLI's `pds check` driver.
//! - [`analyze`] — run the static checks over an already-parsed [`Module`], so
//!   the LSP can reuse one parse for diagnostics, hover, and definition.
//! - [`check_workspace`] — the multi-module counterpart of [`check`]: each
//!   module's parse + static errors plus the cross-module visibility checks
//!   (§8.2).
//! - [`Model`] / [`Workspace`] — the resolved symbol table(s), exposed for the
//!   LSP's hover/go-to-definition and cross-module resolution.
//! - [`graph`] / [`Graph`] — the resolved relationship graph (§9): the nodes,
//!   typed edges, and per-callable sequence traces the `pseudoscript-emit`
//!   crate projects diagram views from.
//!
//! # Example
//!
//! ```
//! use pseudoscript_model::check;
//!
//! let diagnostics = check("//! example\npublic system Banking;");
//! assert!(diagnostics.is_empty());
//! ```

mod check;
pub mod complete;
pub mod fold;
mod graph;
mod model;
pub mod resolve;
pub mod semantic;

pub use complete::{CompletionItem, CompletionKind, completion};
pub use fold::{FoldRange, folding_ranges};
pub use graph::{
    Edge, EdgeKind, Graph, GraphNode, NodeDoc, NodeKind, SigParam, Signature, Step, Trigger,
    Visibility,
};
pub use model::{
    Alias, Member, MemberKind, Model, ModuleEntry, Resolution, Symbol, SymbolKind, Workspace,
};
pub use pseudoscript_syntax::ast;
pub use pseudoscript_syntax::{Diagnostic, Severity};
pub use semantic::{SemKind, SemToken, semantic_tokens};

use pseudoscript_syntax::ast::Module;

/// The closed, built-in macro namespace (`LANG.md` §2.4, ADR-015). Every
/// built-in macro targets callables, so the target set is implicit. Shared by
/// the static checker and the language server's completion.
pub const BUILTIN_MACROS: [&str; 4] = ["onevent", "schedule", "http", "manual"];

/// One module of a multi-module workspace: its caller-supplied FQN and source
/// text (`LANG.md` §8.1).
///
/// The loader derives the FQN from the file path relative to `pds.toml`; this
/// crate stays pure over in-memory modules and never touches the filesystem.
#[derive(Debug, Clone)]
pub struct WorkspaceModule {
    /// The module's fully-qualified name (`banking::core`).
    pub fqn: String,
    /// The module's `.pds` source text.
    pub source: String,
}

impl WorkspaceModule {
    /// Builds a module from its FQN and source.
    pub fn new(fqn: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            fqn: fqn.into(),
            source: source.into(),
        }
    }
}

/// Parses and statically analyzes a whole workspace, returning every
/// diagnostic: each module's parse errors and per-module static errors,
/// followed by the cross-module visibility diagnostics (`LANG.md` §8.2).
///
/// This is the multi-module counterpart of [`check`]; [`check`]/[`analyze`]
/// remain the single-module convenience that derives the module FQN from `//!`.
#[must_use]
pub fn check_workspace(modules: &[WorkspaceModule]) -> Vec<Diagnostic> {
    check_workspace_with_externals(modules, &[])
}

/// Parses and analyzes a workspace that has git dependencies (`LANG.md` §8.4):
/// `local` modules are checked as in [`check_workspace`], while `external`
/// modules — the loader supplies them dependency-name-prefixed — contribute only
/// their `public` symbols so cross-workspace references resolve. External
/// modules are not themselves checked; a dependency's internal diagnostics are
/// not the consumer's.
#[must_use]
pub fn check_workspace_with_externals(
    local: &[WorkspaceModule],
    external: &[WorkspaceModule],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut local_parsed = Vec::with_capacity(local.len());
    for module in local {
        let result = pseudoscript_syntax::parse(&module.source);
        diagnostics.extend(result.diagnostics);
        local_parsed.push((module.fqn.clone(), result.ast));
    }
    let external_parsed = external
        .iter()
        .map(|m| (m.fqn.clone(), pseudoscript_syntax::parse(&m.source).ast));
    let workspace = Workspace::build_with_externals(local_parsed, external_parsed);
    diagnostics.extend(check::run_workspace(&workspace));
    diagnostics
}

/// One workspace module's diagnostics, attributed to its FQN.
///
/// Every span lies in that module's own source, so a tool with the FQN → file
/// mapping (the LSP, `pds check`) can publish each list against the right file.
#[derive(Debug, Clone)]
pub struct ModuleDiagnostics {
    /// The module's fully-qualified name.
    pub fqn: String,
    /// Parse errors, per-module static errors, and the cross-module references
    /// that originate in this module — in that order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses and analyzes a workspace, returning each module's diagnostics keyed
/// by FQN (the per-file counterpart of [`check_workspace`]).
///
/// Unlike [`check_workspace`], which flattens every diagnostic into one list,
/// this preserves attribution: a cross-module visibility error (§8.2) lands in
/// the module that *makes* the offending reference, where its span points.
#[must_use]
pub fn check_workspace_modules(modules: &[WorkspaceModule]) -> Vec<ModuleDiagnostics> {
    let mut parsed = Vec::with_capacity(modules.len());
    let mut parse_diagnostics = Vec::with_capacity(modules.len());
    for module in modules {
        let result = pseudoscript_syntax::parse(&module.source);
        parse_diagnostics.push(result.diagnostics);
        parsed.push((module.fqn.clone(), result.ast));
    }
    let workspace = Workspace::build(parsed);
    workspace
        .modules()
        .iter()
        .zip(parse_diagnostics)
        .map(|(entry, parse)| {
            let mut diagnostics = parse;
            diagnostics.extend(check::run_module(&workspace, entry));
            ModuleDiagnostics {
                fqn: entry.fqn.clone(),
                diagnostics,
            }
        })
        .collect()
}

/// The static diagnostics for the module named `fqn` within an already-built
/// [`Workspace`] — per-module checks plus the cross-module references that
/// originate in it (§8.2). Parse diagnostics are the caller's to prepend.
///
/// Lets a long-lived host (the LSP) resolve the workspace once and query each
/// module's diagnostics without re-parsing.
#[must_use]
pub fn static_diagnostics(workspace: &Workspace, fqn: &str) -> Vec<Diagnostic> {
    workspace
        .module(fqn)
        .map(|entry| check::run_module(workspace, entry))
        .unwrap_or_default()
}

/// Builds the resolved [`Graph`] for a workspace of `(fqn, source)` modules.
///
/// A pure projection of the parsed and resolved modules — no I/O — so the
/// generation crate and a future salsa/LSP layer can both adopt it. Parse and
/// static diagnostics are *not* surfaced here; run [`check_workspace`] for
/// those. The graph records what resolves and annotates what does not.
#[must_use]
pub fn graph(modules: &[WorkspaceModule]) -> Graph {
    let parsed = modules
        .iter()
        .map(|m| (m.fqn.clone(), pseudoscript_syntax::parse(&m.source).ast));
    Graph::build(&Workspace::build(parsed))
}

/// Parses and statically analyzes `src`, returning every diagnostic: the parse
/// errors from [`pseudoscript_syntax`] followed by the static-analysis errors.
#[must_use]
pub fn check(src: &str) -> Vec<Diagnostic> {
    let parsed = pseudoscript_syntax::parse(src);
    let mut diagnostics = parsed.diagnostics;
    diagnostics.extend(analyze(&parsed.ast));
    diagnostics
}

/// Runs the static checks over an already-parsed module.
///
/// Returns only the static diagnostics; the caller supplies the parse
/// diagnostics. The LSP uses this to reuse a single parse across features.
#[must_use]
pub fn analyze(module: &Module) -> Vec<Diagnostic> {
    let model = Model::build(module);
    check::run(module, &model)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cross-module reference to a private node is reported against the module
    /// that *makes* the reference, not the one declaring the node (§8.2).
    #[test]
    fn cross_module_diagnostic_attributed_to_referrer() {
        let modules = [
            WorkspaceModule::new("a", "//! a\n\nsystem Hidden;\n"),
            WorkspaceModule::new("b", "//! b\n\npublic container C for a::Hidden;\n"),
        ];
        let by_module = check_workspace_modules(&modules);

        let a = by_module.iter().find(|m| m.fqn == "a").expect("module a");
        let b = by_module.iter().find(|m| m.fqn == "b").expect("module b");

        assert!(
            a.diagnostics.is_empty(),
            "declarer is clean: {:?}",
            a.diagnostics
        );
        assert!(
            b.diagnostics.iter().any(|d| d.message.contains("private")),
            "referrer flags the private access: {:?}",
            b.diagnostics
        );
    }

    /// A consumer references a dependency's `public` node by its
    /// dependency-rooted FQN; the reference resolves and the dependency's own
    /// modules are not checked (§8.4).
    #[test]
    fn cross_workspace_reference_to_public_dep_node_resolves() {
        let local = [WorkspaceModule::new(
            "main",
            "//! main\n\npublic container Portal for auth::core::Login;\n",
        )];
        let external = [WorkspaceModule::new(
            "auth::core",
            "//! auth core\n\npublic system Login;\n",
        )];
        let diagnostics = check_workspace_with_externals(&local, &external);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    /// Referencing a dependency's *private* node is rejected (§8.4 extends §8.2).
    #[test]
    fn cross_workspace_reference_to_private_dep_node_is_rejected() {
        let local = [WorkspaceModule::new(
            "main",
            "//! main\n\npublic container Portal for auth::core::Login;\n",
        )];
        let external = [WorkspaceModule::new(
            "auth::core",
            "//! auth core\n\nsystem Login;\n",
        )];
        let diagnostics = check_workspace_with_externals(&local, &external);
        assert!(
            diagnostics.iter().any(|d| d.message.contains("private")),
            "{diagnostics:?}"
        );
    }

    /// A dangling reference into a known dependency module is rejected (§8.5).
    #[test]
    fn dangling_cross_workspace_reference_is_rejected() {
        let local = [WorkspaceModule::new(
            "main",
            "//! main\n\npublic container Portal for auth::core::Ghost;\n",
        )];
        let external = [WorkspaceModule::new(
            "auth::core",
            "//! auth core\n\npublic system Login;\n",
        )];
        let diagnostics = check_workspace_with_externals(&local, &external);
        assert!(
            diagnostics.iter().any(|d| d.message.contains("dangling")),
            "{diagnostics:?}"
        );
    }

    /// A well-formed multi-module workspace produces no diagnostics anywhere.
    #[test]
    fn clean_workspace_has_no_diagnostics() {
        let modules = [
            WorkspaceModule::new("a", "//! a\n\npublic system Svc;\n"),
            WorkspaceModule::new("b", "//! b\n\npublic container C for a::Svc;\n"),
        ];
        assert!(
            check_workspace_modules(&modules)
                .iter()
                .all(|m| m.diagnostics.is_empty())
        );
    }
}
