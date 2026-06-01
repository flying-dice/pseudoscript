//! Foldable source regions — the shared engine behind the LSP
//! (`pseudoscript-lsp`) and the web IDE (`pseudoscript-wasm`).
//!
//! Every *disclosed* declaration (one with a `{ }` body) and every statement
//! block is foldable. Ranges are absolute byte offsets of the whole construct;
//! the LSP maps them to line-based `FoldingRange`s, the IDE to editor offsets.

use pseudoscript_syntax::{Span, ast, parse};
use serde::Serialize;

/// A foldable region in absolute byte offsets (the construct from its opening
/// keyword/brace through its closing brace).
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
/// to fold.
fn collect_decl_spans(decl: &ast::Decl, out: &mut Vec<Span>) {
    match &decl.kind {
        ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node) => {
            let Some(body) = &node.body else { return };
            out.push(decl.span);
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
        ast::DeclKind::Data(data) => {
            if !matches!(data.body, ast::DataBody::BlackBox) {
                out.push(decl.span);
            }
        }
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
}
