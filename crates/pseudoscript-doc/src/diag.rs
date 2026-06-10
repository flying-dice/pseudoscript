//! Host-prepared diagnostics for the doc build.
//!
//! The renderer never sees module sources, so the host positions each finding
//! once — 1-based line/column from the byte span — before handing the set in.
//! Both hosts (the CLI and the IDE wasm session) call [`prepare_diagnostics`]
//! with the same per-module check output, so the health page is identical
//! wherever the site is built.

use pseudoscript_model::{ModuleDiagnostics, WorkspaceModule};
use pseudoscript_syntax::Severity;

/// One positioned diagnostic, ready for the doc build.
#[derive(Debug, Clone)]
pub struct DiagnosticInput {
    /// The declaring module FQN.
    pub module: String,
    /// `"error"` or `"warning"`.
    pub severity: String,
    /// The stable rule code (e.g. `PDS-ARCH-001`), when the diagnostic has one.
    pub code: Option<String>,
    /// The rule's principle-article URL, when published.
    pub code_url: Option<String>,
    /// The finding message.
    pub message: String,
    /// 1-based source line.
    pub line: u32,
    /// 1-based source column.
    pub column: u32,
    /// The span's starting byte offset in the module source.
    pub start: u32,
}

/// Positions raw per-module diagnostics in their sources: each finding gains
/// its 1-based line/column, carrying severity, rule code, and article URL
/// through. A diagnostic whose module is absent from `modules` keeps offset
/// `0:1:1` rather than being dropped — a finding is never silently lost.
#[must_use]
pub fn prepare_diagnostics(
    modules: &[WorkspaceModule],
    per_module: &[ModuleDiagnostics],
) -> Vec<DiagnosticInput> {
    per_module
        .iter()
        .flat_map(|m| {
            let source = modules
                .iter()
                .find(|module| module.fqn == m.fqn)
                .map(|module| module.source.as_str());
            m.diagnostics.iter().map(move |d| {
                let start = d.span.start;
                let (line, column) =
                    source.map_or((1, 1), |src| line_col(src, d.span.start as usize));
                DiagnosticInput {
                    module: m.fqn.clone(),
                    severity: severity_word(d.severity).to_owned(),
                    code: d.code.clone(),
                    code_url: d.code_description.clone(),
                    message: d.message.clone(),
                    line,
                    column,
                    start,
                }
            })
        })
        .collect()
}

/// The severity word the site renders.
fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// The 1-based line/column of byte `offset` in `source`.
fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let column = before.rfind('\n').map_or(offset, |nl| offset - nl - 1) + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_is_one_based_and_newline_aware() {
        let src = "ab\ncd\nef";
        assert_eq!(line_col(src, 0), (1, 1));
        assert_eq!(line_col(src, 4), (2, 2));
        assert_eq!(line_col(src, 6), (3, 1));
        assert_eq!(line_col(src, 99), (3, 3), "clamped to the end");
    }
}
