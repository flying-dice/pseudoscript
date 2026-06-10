//! Pure language logic, free of any server or I/O.
//!
//! Each function maps source text (and, for cursor features, an LSP position)
//! to an LSP value. The the server layer only owns the document store and
//! the protocol plumbing; everything testable lives here.

use lsp_types::{
    CodeDescription, Diagnostic, DiagnosticSeverity, Hover, HoverContents, InlayHint,
    InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind, Position, TextEdit, Url,
};
use pseudoscript_format::format;
use pseudoscript_model::{Workspace, check};
use pseudoscript_syntax::{LineIndex, Span, TokenKind, tokenize};

use crate::convert::{full_range, offset_to_position, position_to_offset, span_to_range};
use crate::resolve::resolve_at;

pub use crate::complete::completion;
pub use crate::refs::{Occurrence, highlights, references};
pub use crate::semantic::{legend as semantic_legend, semantic_tokens};
pub use crate::symbols::{document_symbols, folding_ranges, workspace_symbols};

/// The editor range of the identifier under `position`, if it resolves to a
/// renameable symbol. Backs `textDocument/prepareRename`.
#[must_use]
pub fn renameable(
    ws: &Workspace,
    from_fqn: &str,
    src: &str,
    position: Position,
) -> Option<lsp_types::Range> {
    let offset = position_to_offset(src, position);
    let hit = resolve_at(ws, from_fqn, src, offset)?;
    let index = LineIndex::new(src);
    Some(span_to_range(src, &index, hit.clicked))
}

/// A go-to-definition target: the module the definition lives in and its span
/// in that module's source. The server maps `fqn` to a file URI.
#[derive(Debug, Clone)]
pub struct DefTarget {
    /// FQN of the module containing the definition.
    pub fqn: String,
    /// Span of the definition within that module's source.
    pub span: Span,
}

/// Resolves the identifier at `position` in module `from_fqn` to its definition
/// — across files and into `self.`/node members.
#[must_use]
pub fn definition(
    ws: &Workspace,
    from_fqn: &str,
    src: &str,
    position: Position,
) -> Option<DefTarget> {
    let offset = position_to_offset(src, position);
    let hit = resolve_at(ws, from_fqn, src, offset)?;
    Some(DefTarget {
        fqn: hit.target_module,
        span: hit.target_span,
    })
}

/// Builds hover content for the identifier at `position`: its kind/FQN (or
/// member signature) and any `///` summary. Workspace-aware.
#[must_use]
#[tracing::instrument(level = "debug", skip(ws, src))]
pub fn hover(ws: &Workspace, from_fqn: &str, src: &str, position: Position) -> Option<Hover> {
    let offset = position_to_offset(src, position);
    let index = LineIndex::new(src);

    if let Some(hit) = resolve_at(ws, from_fqn, src, offset) {
        let mut text = format!("**{}**", hit.title);
        if let Some(body) = &hit.body {
            text.push_str("\n\n");
            text.push_str(body);
        }
        return Some(markup_hover(text, span_to_range(src, &index, hit.clicked)));
    }

    // Fallback: a local binding shows its inferred type (`tokens: Token[]`).
    let (name, span) = ident_at(src, offset)?;
    let local = pseudoscript_model::infer::binding_type_at(ws, from_fqn, &name)?;
    Some(markup_hover(
        format!("**`{name}: {}`**\n\nlocal binding", local.ty),
        span_to_range(src, &index, span),
    ))
}

/// Inlay hints: an inferred `: Type` after each untyped local binding.
#[must_use]
pub fn inlay_hints(ws: &Workspace, from_fqn: &str, src: &str) -> Vec<InlayHint> {
    let index = LineIndex::new(src);
    pseudoscript_model::infer::local_types(ws, from_fqn)
        .into_iter()
        .map(|local| InlayHint {
            position: offset_to_position(src, &index, local.name_span.end),
            label: InlayHintLabel::String(format!(": {}", local.ty)),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(false),
            padding_right: Some(false),
            data: None,
        })
        .collect()
}

/// A Markdown hover with a range.
fn markup_hover(value: String, range: lsp_types::Range) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(range),
    }
}

/// The identifier token text and span at `offset` (caret-just-past tolerant).
fn ident_at(src: &str, offset: u32) -> Option<(String, Span)> {
    let pick = |o: u32| {
        tokenize(src)
            .into_iter()
            .find(|t| t.kind == TokenKind::Ident && t.span.start <= o && o < t.span.end)
            .map(|t| (t.text, t.span))
    };
    pick(offset).or_else(|| offset.checked_sub(1).and_then(pick))
}

/// Maps a [`pseudoscript_syntax::Severity`] to its LSP counterpart.
fn severity(severity: pseudoscript_syntax::Severity) -> DiagnosticSeverity {
    match severity {
        pseudoscript_syntax::Severity::Error => DiagnosticSeverity::ERROR,
        pseudoscript_syntax::Severity::Warning => DiagnosticSeverity::WARNING,
        pseudoscript_syntax::Severity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// Runs the single-module check pipeline over `src` and maps every diagnostic
/// into the LSP shape. The workspace path uses [`lsp_diagnostics`] directly.
#[must_use]
pub fn diagnostics(src: &str) -> Vec<Diagnostic> {
    lsp_diagnostics(src, &check(src))
}

/// Maps model diagnostics whose spans lie in `src` into the LSP shape,
/// converting byte spans to UTF-16 ranges.
#[must_use]
pub fn lsp_diagnostics(src: &str, diags: &[pseudoscript_syntax::Diagnostic]) -> Vec<Diagnostic> {
    let index = LineIndex::new(src);
    diags
        .iter()
        .map(|d| Diagnostic {
            range: span_to_range(src, &index, d.span),
            severity: Some(severity(d.severity)),
            code: d.code.clone().map(lsp_types::NumberOrString::String),
            code_description: d
                .code_description
                .as_deref()
                .and_then(|url| Url::parse(url).ok())
                .map(|href| CodeDescription { href }),
            source: Some("pseudoscript".to_owned()),
            message: d.message.clone(),
            ..Diagnostic::default()
        })
        .collect()
}

/// Produces a whole-document [`TextEdit`] that rewrites `src` to its canonical
/// form, or `None` when `src` does not parse (the buffer is then left intact).
#[must_use]
pub fn format_edit(src: &str) -> Option<TextEdit> {
    let formatted = format(src).ok()?;
    let index = LineIndex::new(src);
    Some(TextEdit {
        range: full_range(src, &index),
        new_text: formatted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REBIND: &str =
        "//! example\n\npublic data Info { id: number }\n\npublic system Banking;\n";

    #[test]
    fn well_formed_source_has_no_diagnostics() {
        assert!(diagnostics(REBIND).is_empty());
    }

    #[test]
    fn parse_error_yields_error_diagnostic() {
        let diags = diagnostics("public system ;");
        assert!(
            diags
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        );
    }

    #[test]
    fn format_edit_present_for_valid_input() {
        let edit = format_edit("public   system   Banking ;").expect("valid input formats");
        assert_eq!(edit.new_text, "public system Banking;\n");
    }

    #[test]
    fn format_edit_absent_for_parse_error() {
        assert!(format_edit("public system ;").is_none());
    }
}
