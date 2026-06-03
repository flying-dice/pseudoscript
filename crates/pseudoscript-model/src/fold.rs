//! Foldable source regions — the shared engine behind the LSP
//! (`pseudoscript-lsp`) and the web IDE (`pseudoscript-ide`).
//!
//! Every brace-delimited declaration (a node or `data` record) and every
//! statement block is foldable; brace-less `data = | …` unions and black-box
//! `;` declarations are not. Ranges run from the construct's name/header through
//! its closing brace — never over the leading doc comments — in absolute byte
//! offsets; the LSP maps them to line-based `FoldingRange`s, the IDE to editor
//! offsets.

use pseudoscript_syntax::{Span, ast, parse};
use serde::Serialize;

/// A foldable region in absolute byte offsets (from the construct's name/header
/// through its closing brace).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FoldRange {
    /// Start byte offset of the construct.
    pub start: u32,
    /// End byte offset of the construct.
    pub end: u32,
}

/// The foldable regions of `src`: every multi-line declaration and statement
/// block. Order follows source position of the collected spans.
#[must_use]
pub fn folding_ranges(src: &str) -> Vec<FoldRange> {
    let module = parse(src).ast;
    let mut spans = Vec::new();
    for item in &module.items {
        if let ast::Item::Decl(decl) = item {
            collect_decl_spans(decl, &mut spans);
        }
    }
    spans
        .into_iter()
        .map(|s| FoldRange {
            start: s.start,
            end: s.end,
        })
        .collect()
}

/// Collects the foldable span of a *disclosed* declaration (one with a `{ }`
/// body) and any blocks nested in it. A black-box `;` declaration has nothing
/// to fold; a brace-less `data = | …` union is not a block fold.
///
/// The span runs from the declaration's *name* (the header line) to its end, so
/// the fold keeps the header and any leading `///` doc comments visible —
/// `decl.span` itself reaches back over that leading trivia.
fn collect_decl_spans(decl: &ast::Decl, out: &mut Vec<Span>) {
    match &decl.kind {
        ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node) => {
            let Some(body) = &node.body else { return };
            out.push(head_span(node.name.span, decl.span));
            for member in body {
                match member {
                    ast::BodyMember::Decl(inner) => collect_decl_spans(inner, out),
                    ast::BodyMember::Callable(callable) => {
                        if let Some(block) = &callable.body {
                            collect_block_spans(block, out);
                        }
                    }
                }
            }
        }
        // Only a brace-delimited record folds; a `| …` union has no body block.
        ast::DeclKind::Data(data) => {
            if matches!(data.body, ast::DataBody::Record(_)) {
                out.push(head_span(data.name.span, decl.span));
            }
        }
    }
}

/// A fold span from the start of `name` to the end of `decl`, excluding the
/// leading doc comments and blank lines `decl.span` reaches back over.
fn head_span(name: Span, decl: Span) -> Span {
    Span {
        start: name.start,
        end: decl.end,
    }
}

/// Collects a block's span and the spans of any nested control-flow blocks.
fn collect_block_spans(block: &ast::Block, out: &mut Vec<Span>) {
    out.push(block.span);
    for stmt in &block.stmts {
        match &stmt.kind {
            ast::StmtKind::If {
                then_block,
                else_block,
                ..
            } => {
                collect_block_spans(then_block, out);
                if let Some(block) = else_block {
                    collect_block_spans(block, out);
                }
            }
            ast::StmtKind::For { body, .. } | ast::StmtKind::While { body, .. } => {
                collect_block_spans(body, out);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_disclosed_node_and_nested_block() {
        let src = "//! m\n\nsystem S {\n  run() {\n    return self\n  }\n}\n";
        let ranges = folding_ranges(src);
        // the system declaration and the callable's block
        assert!(ranges.len() >= 2, "{ranges:?}");
        let outer_close = (src.rfind('}').unwrap() + 1) as u32;
        // the widest range covers the whole disclosed system, brace to brace
        assert!(
            ranges
                .iter()
                .any(|r| r.start <= src.find('{').unwrap() as u32 && r.end >= outer_close),
            "{ranges:?}"
        );
    }

    #[test]
    fn black_box_decl_has_no_fold() {
        let src = "//! m\n\nsystem S;\n";
        assert!(folding_ranges(src).is_empty());
    }

    #[test]
    fn fold_starts_at_the_declaration_not_its_doc_comment() {
        // A leading `///` doc comment (and the blank line above it) must stay
        // outside the fold — it is not swallowed into the body.
        let src = "//! m\n\n/// doc.\npublic data D {\n  x: number\n}\n";
        let ranges = folding_ranges(src);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        // the fold begins at `public data D`, after the doc comment
        let decl = src.find("public data D").unwrap() as u32;
        let doc = src.find("/// doc.").unwrap() as u32;
        assert!(
            ranges[0].start >= decl,
            "fold reaches into the doc comment: {ranges:?}"
        );
        assert!(ranges[0].start > doc, "{ranges:?}");
    }

    #[test]
    fn brace_less_union_does_not_fold() {
        let src = "//! m\n\npublic data U =\n  | A\n  | B\n";
        assert!(
            folding_ranges(src).is_empty(),
            "a `| …` union is not a block fold"
        );
    }
}
