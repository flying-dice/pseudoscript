//! Local-binding type inference, for inlay hints and hover.
//!
//! `PseudoScript` assignments are untyped (`x = expr`). This module infers a
//! binding's type from its right-hand side — a call's return type, a field
//! access, a `from` expression, or a literal — so the editor can show
//! `x: Token[]` without the author writing it. Inference is best-effort: an
//! expression it can't type yields no hint rather than a guess.

use crate::{Workspace, ast};
use pseudoscript_syntax::Span;

use crate::resolve::{enclosing_node, resolve_owner};

/// An inferred local binding: its name, the span of that name, and its type.
pub struct LocalType {
    pub name: String,
    pub name_span: Span,
    pub ty: String,
}

/// Every typed local binding in module `from_fqn` (assignments whose right-hand
/// side has an inferable type).
#[must_use]
pub fn local_types(ws: &Workspace, from_fqn: &str) -> Vec<LocalType> {
    let Some(entry) = ws.module(from_fqn) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in &entry.ast.items {
        if let ast::Item::Decl(decl) = item {
            walk_decl(ws, from_fqn, decl, &mut out);
        }
    }
    out
}

/// The inferred type of the binding named by the identifier at `offset`, with
/// its name — for hover. Matches by binding name, so a use resolves to its
/// declaration's inferred type.
#[must_use]
pub fn binding_type_at(ws: &Workspace, from_fqn: &str, name: &str) -> Option<LocalType> {
    local_types(ws, from_fqn)
        .into_iter()
        .find(|lt| lt.name == name)
}

fn walk_decl(ws: &Workspace, from_fqn: &str, decl: &ast::Decl, out: &mut Vec<LocalType>) {
    let (ast::DeclKind::Person(node)
    | ast::DeclKind::System(node)
    | ast::DeclKind::Container(node)
    | ast::DeclKind::Component(node)) = &decl.kind
    else {
        return;
    };
    for member in node.body.iter().flatten() {
        match member {
            ast::BodyMember::Callable(callable) => {
                if let Some(block) = &callable.body {
                    walk_block(ws, from_fqn, block, out);
                }
            }
            ast::BodyMember::Decl(inner) => walk_decl(ws, from_fqn, inner, out),
        }
    }
}

fn walk_block(ws: &Workspace, from_fqn: &str, block: &ast::Block, out: &mut Vec<LocalType>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            ast::StmtKind::Assign { name, value } => {
                if let Some(ty) = expr_type(ws, from_fqn, value) {
                    out.push(LocalType {
                        name: name.name.clone(),
                        name_span: name.span,
                        ty,
                    });
                }
            }
            ast::StmtKind::For {
                binding,
                iter,
                body,
            } => {
                // `for (x in xs)` binds `x` to the element type of an array `xs`.
                if let Some(ty) = expr_type(ws, from_fqn, iter)
                    && let Some(element) = ty.strip_suffix("[]")
                {
                    out.push(LocalType {
                        name: binding.name.clone(),
                        name_span: binding.span,
                        ty: element.to_owned(),
                    });
                }
                walk_block(ws, from_fqn, body, out);
            }
            ast::StmtKind::If {
                then_block,
                else_block,
                ..
            } => {
                walk_block(ws, from_fqn, then_block, out);
                if let Some(block) = else_block {
                    walk_block(ws, from_fqn, block, out);
                }
            }
            ast::StmtKind::While { body, .. } => walk_block(ws, from_fqn, body, out),
            ast::StmtKind::Return(_) | ast::StmtKind::Expr(_) => {}
        }
    }
}

/// The inferred type of an expression, or `None` when it can't be typed.
fn expr_type(ws: &Workspace, from_fqn: &str, expr: &ast::Expr) -> Option<String> {
    match &expr.kind {
        ast::ExprKind::Literal(ast::Literal::String { .. }) => Some("string".to_owned()),
        ast::ExprKind::Literal(ast::Literal::Number { .. }) => Some("number".to_owned()),
        ast::ExprKind::Literal(ast::Literal::Bool { .. }) => Some("bool".to_owned()),
        ast::ExprKind::Paren(inner) => expr_type(ws, from_fqn, inner),
        // `T from { .. }` produces a value of the named type.
        ast::ExprKind::From { ty, .. } => ty.segments.last().map(|seg| seg.name.clone()),
        ast::ExprKind::Postfix { base, segments } => postfix_type(ws, from_fqn, base, segments),
        _ => None,
    }
}

/// The type of a postfix chain: walk each `.member`, the result is the last
/// member's type. Each non-final member's type must itself name a node/data so
/// the chain can continue.
fn postfix_type(
    ws: &Workspace,
    from_fqn: &str,
    base: &ast::Expr,
    segments: &[ast::PostfixSeg],
) -> Option<String> {
    let mut owner = base_owner(ws, from_fqn, base)?;
    let mut result = None;
    for (i, seg) in segments.iter().enumerate() {
        let member_ty = ws
            .module(&owner.0)?
            .model
            .members(&owner.1)
            .iter()
            .find(|m| m.name == seg.name.name)?
            .ty
            .clone();
        result = Some(member_ty.clone());
        if i + 1 < segments.len() {
            owner = type_owner(ws, from_fqn, &member_ty)?;
        }
    }
    result
}

/// The node/data owner a postfix base denotes: `self`'s enclosing node, or the
/// node/data a path names.
fn base_owner(ws: &Workspace, from_fqn: &str, base: &ast::Expr) -> Option<(String, String)> {
    match &base.kind {
        ast::ExprKind::Ref(ast::Ref::SelfNode(span)) => {
            let node = enclosing_node(&ws.module(from_fqn)?.ast, span.start)?;
            Some((from_fqn.to_owned(), node))
        }
        ast::ExprKind::Ref(ast::Ref::Path(path)) => {
            let segments: Vec<&str> = path.segments.iter().map(|s| s.name.as_str()).collect();
            resolve_owner(ws, from_fqn, &segments)
        }
        ast::ExprKind::Paren(inner) => base_owner(ws, from_fqn, inner),
        _ => None,
    }
}

/// The node/data named by a type string (`Token[]` → `Token`, `a::T<U>` → `a::T`),
/// for continuing a postfix chain.
#[must_use]
pub fn type_owner(ws: &Workspace, from_fqn: &str, ty: &str) -> Option<(String, String)> {
    let base = ty.trim_end_matches("[]");
    let base = base.split('<').next().unwrap_or(base).trim();
    let segments: Vec<&str> = base.split("::").collect();
    resolve_owner(ws, from_fqn, &segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_syntax::parse;

    fn ws(modules: &[(&str, &str)]) -> Workspace {
        Workspace::build(modules.iter().map(|(f, s)| ((*f).to_owned(), parse(s).ast)))
    }

    // Mirrors the `syntax` module: `tokens` is the `Token[]` returned by
    // `Lexer.tokenize`, reached through the container-qualified `Syntax::Lexer`.
    const SYNTAX: &str = "//! syntax\n\n\
        public data Token { kind: string }\n\n\
        public component Lexer for Syntax {\n  tokenize(text: string): Token[];\n}\n\n\
        public component Parser for Syntax {\n  \
        public parse(text: string): string {\n    \
        tokens = Syntax::Lexer.tokenize(text)\n    return tokens.kind\n  }\n}\n";

    #[test]
    fn infers_local_from_member_call_return() {
        let workspace = ws(&[("syntax", SYNTAX)]);
        let locals = local_types(&workspace, "syntax");
        let tokens = locals
            .iter()
            .find(|l| l.name == "tokens")
            .expect("`tokens` typed");
        assert_eq!(tokens.ty, "Token[]");
    }

    #[test]
    fn binding_type_at_resolves_by_name() {
        let workspace = ws(&[("syntax", SYNTAX)]);
        let local = binding_type_at(&workspace, "syntax", "tokens").expect("tokens");
        assert_eq!(local.ty, "Token[]");
    }

    #[test]
    fn infers_literal_and_from_expression() {
        let src = "//! m\n\nsystem S {\n  go() {\n    n = 42\n    p = Parsed from { n }\n  }\n}\n\npublic data Parsed { x: number }\n";
        let workspace = ws(&[("m", src)]);
        let locals = local_types(&workspace, "m");
        assert_eq!(locals.iter().find(|l| l.name == "n").unwrap().ty, "number");
        assert_eq!(locals.iter().find(|l| l.name == "p").unwrap().ty, "Parsed");
    }
}
