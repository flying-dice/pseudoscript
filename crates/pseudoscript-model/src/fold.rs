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

/// What kind of construct a fold covers, so an editor can pick a default fold
/// state per kind — e.g. collapse a callable's `Member` impl block on open
/// while leaving the structural `Node` bodies expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FoldKind {
    /// A structural node body (`person`/`system`/`container`/`component`).
    Node,
    /// A callable's implementation block — a member impl.
    Member,
    /// A `data` record body.
    Data,
    /// A nested control-flow scope (`if`/`for`/`while`) inside a member.
    Block,
}

/// A foldable region in absolute byte offsets (from the construct's name/header
/// through its closing brace), with the kind of construct it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FoldRange {
    /// Start byte offset of the construct.
    pub start: u32,
    /// End byte offset of the construct.
    pub end: u32,
    /// The kind of construct this fold covers.
    pub kind: FoldKind,
}

/// The foldable regions of `src`: every multi-line declaration and statement
/// block. Order follows source position of the collected spans.
#[must_use]
pub fn folding_ranges(src: &str) -> Vec<FoldRange> {
    let module = parse(src).ast;
    let mut out = Vec::new();
    for item in &module.items {
        if let ast::Item::Decl(decl) = item {
            collect_decl_spans(decl, &mut out);
        }
    }
    out
}

/// Pushes a fold of `span` with `kind`.
fn push(out: &mut Vec<FoldRange>, span: Span, kind: FoldKind) {
    out.push(FoldRange {
        start: span.start,
        end: span.end,
        kind,
    });
}

/// Collects the foldable span of a *disclosed* declaration (one with a `{ }`
/// body) and any blocks nested in it. A black-box `;` declaration has nothing
/// to fold; a brace-less `data = | …` union is not a block fold.
///
/// The span runs from the declaration's *name* (the header line) to its end, so
/// the fold keeps the header and any leading `///` doc comments visible —
/// `decl.span` itself reaches back over that leading trivia.
fn collect_decl_spans(decl: &ast::Decl, out: &mut Vec<FoldRange>) {
    match &decl.kind {
        ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node) => {
            let Some(body) = &node.body else { return };
            push(out, head_span(node.name.span, decl.span), FoldKind::Node);
            for member in body {
                match member {
                    ast::BodyMember::Decl(inner) => collect_decl_spans(inner, out),
                    ast::BodyMember::Callable(callable) => {
                        if let Some(block) = &callable.body {
                            // The callable's own body is the member impl block;
                            // its nested control-flow scopes are `Block` folds.
                            push(out, block.span, FoldKind::Member);
                            collect_nested_blocks(block, out);
                        }
                    }
                }
            }
        }
        // Only a brace-delimited record folds; a `| …` union has no body block.
        ast::DeclKind::Data(data) => {
            if matches!(data.body, ast::DataBody::Record(_)) {
                push(out, head_span(data.name.span, decl.span), FoldKind::Data);
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

/// Collects the spans of control-flow blocks nested *inside* `block` (not
/// `block` itself — its caller folds that). Each `if`/`for`/`while` scope is a
/// `Block` fold.
fn collect_nested_blocks(block: &ast::Block, out: &mut Vec<FoldRange>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            ast::StmtKind::If {
                then_block,
                else_block,
                ..
            } => {
                push(out, then_block.span, FoldKind::Block);
                collect_nested_blocks(then_block, out);
                if let Some(block) = else_block {
                    push(out, block.span, FoldKind::Block);
                    collect_nested_blocks(block, out);
                }
            }
            ast::StmtKind::For { body, .. } | ast::StmtKind::While { body, .. } => {
                push(out, body.span, FoldKind::Block);
                collect_nested_blocks(body, out);
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
    fn tags_node_member_and_nested_block_kinds() {
        let src =
            "//! m\n\nsystem S {\n  run() {\n    if (cond) {\n      return self\n    }\n  }\n}\n";
        let kinds: Vec<FoldKind> = folding_ranges(src).iter().map(|r| r.kind).collect();
        // outer node body, the callable's member impl block, the nested `if` scope
        assert_eq!(
            kinds,
            vec![FoldKind::Node, FoldKind::Member, FoldKind::Block],
            "{kinds:?}"
        );
    }

    #[test]
    fn record_folds_as_data() {
        let src = "//! m\n\npublic data D {\n  x: number\n}\n";
        let ranges = folding_ranges(src);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].kind, FoldKind::Data);
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
