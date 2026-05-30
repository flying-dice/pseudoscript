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
//! cannot be constructed off-wasm). Diagnostics are the foundation an
//! LSP-over-wasm server is built on; hover/completion/definition slot in here
//! and reuse [`pseudoscript_model`] without the native `tower-lsp`/`tokio`
//! transport.

use pseudoscript_emit::{Scene, View, graph_of_source, project, render_svg};
use pseudoscript_format::format as format_source;
use pseudoscript_model::{
    NodeKind, WorkspaceModule, check as check_source, check_workspace_modules,
};
use pseudoscript_syntax::{Diagnostic, LineIndex, Severity, parse as parse_source};
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

/// Lists the nodes declared in `source` as a JSON array of
/// `{ fqn, name, kind, triggered }`. A host uses this to populate a diagram's
/// target picker: `container` views target a `system`, `component` views a
/// `container`, and `sequence` views a `triggered` callable.
#[wasm_bindgen]
#[must_use]
pub fn outline(source: &str) -> String {
    let graph = graph_of_source(source);
    let nodes: Vec<OutlineNode> = graph
        .nodes()
        .iter()
        .map(|n| OutlineNode {
            fqn: n.fqn.clone(),
            name: n.name.clone(),
            kind: n.kind,
            triggered: !n.triggers.is_empty(),
        })
        .collect();
    to_json(&nodes)
}

// ---- logic (host-testable; no `JsError`, which cannot exist off-wasm) ------

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
    let graph = graph_of_source(source);
    let view = match view {
        "context" => View::Context,
        "container" => View::Container {
            of: target.to_owned(),
        },
        "component" => View::Component {
            of: target.to_owned(),
        },
        "sequence" => View::Sequence {
            entry: target.to_owned(),
        },
        other => return Err(format!("unknown view `{other}`")),
    };
    project(&graph, view).map_err(|e| e.to_string())
}

// ---- DTOs ------------------------------------------------------------------

/// One input module for [`check_modules`].
#[derive(Deserialize)]
struct InputModule {
    fqn: String,
    source: String,
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
#[derive(Serialize)]
struct OutlineNode {
    fqn: String,
    name: String,
    kind: NodeKind,
    triggered: bool,
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
    use super::{check, check_modules_impl, format_impl, outline, parse, project_view};

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
    fn check_modules_attributes_to_referrer() {
        let input = r#"[
            {"fqn":"a","source":"//! a\nsystem Hidden;"},
            {"fqn":"b","source":"//! b\npublic container C for a::Hidden;"}
        ]"#;
        let out = check_modules_impl(input).expect("valid input checks");
        assert!(out.contains(r#""fqn":"b""#), "{out}");
    }
}
