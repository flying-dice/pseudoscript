//! Local-binding types and right-hand-side inference, for hover and completion.
//!
//! A binding states its type through a `from` right-hand side (`x = T from expr`,
//! §7.1, ADR-035), so [`local_types`]/[`binding_type_at`] read the `from` target.
//! Right-hand-side inference also types a `for` binding's element and a chain
//! receiver mid-expression ([`owner_at_dot`]). Inference is best-effort — an
//! expression it can't type (a bare call, a field access) yields nothing rather
//! than a guess.

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
                // A binding's type is its `from` target (§7.1, ADR-035); a
                // non-`from` RHS is left untyped.
                if let Some(rendered) = expr_type(ws, from_fqn, value) {
                    out.push(LocalType {
                        name: name.name.clone(),
                        name_span: name.span,
                        ty: rendered,
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

/// The inferred type of an expression, or `None` when it can't be typed —
/// a call's return type, a field access, a `from`, or a literal.
fn expr_type(ws: &Workspace, from_fqn: &str, expr: &ast::Expr) -> Option<String> {
    match &expr.kind {
        ast::ExprKind::Literal(ast::Literal::String { .. }) => Some("string".to_owned()),
        ast::ExprKind::Literal(ast::Literal::Number { .. }) => Some("number".to_owned()),
        ast::ExprKind::Literal(ast::Literal::Bool { .. }) => Some("bool".to_owned()),
        ast::ExprKind::Paren(inner) => expr_type(ws, from_fqn, inner),
        // `T from …` produces a value of the target type (ADR-035).
        ast::ExprKind::From { ty, .. } => Some(crate::model::render_type(ty)),
        ast::ExprKind::Postfix { base, segments } => postfix_type(ws, from_fqn, base, segments),
        // §7.5: arithmetic yields `number`, every other operator yields `bool`.
        ast::ExprKind::Binary { op, .. } => Some(
            match op {
                ast::BinOp::Add
                | ast::BinOp::Sub
                | ast::BinOp::Mul
                | ast::BinOp::Div
                | ast::BinOp::Rem => "number",
                _ => "bool",
            }
            .to_owned(),
        ),
        ast::ExprKind::Unary { op, .. } => Some(
            match op {
                ast::UnaryOp::Not => "bool",
                ast::UnaryOp::Neg => "number",
            }
            .to_owned(),
        ),
        // §3.6: a constant FQN resolves to its declared primitive type.
        ast::ExprKind::Ref(ast::Ref::Path(path)) if !path.is_simple() => {
            let fqn = path
                .segments
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let module = crate::resolve::module_of(&fqn);
            ws.module_model(module)?
                .constant_types()
                .find(|(f, _)| *f == fqn)
                .map(|(_, ty)| ty.to_owned())
        }
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
            .module_model(&owner.0)?
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
            resolve_owner(ws, from_fqn, &segments).or_else(|| {
                // A chain rooted at a local binding: `acc.balance` where `acc`
                // was bound earlier. Resolve the binding's type, then its owner.
                if let [name] = segments.as_slice() {
                    let local = binding_type_at(ws, from_fqn, name)?;
                    type_owner(ws, from_fqn, &local.ty)
                } else {
                    None
                }
            })
        }
        ast::ExprKind::Paren(inner) => base_owner(ws, from_fqn, inner),
        _ => None,
    }
}

/// The node/data owner of the receiver whose trailing `.` sits at byte `dot` —
/// member completion on a chain (`Repo.fetch(id).value.` resolves the type of
/// `value`, so its members can be offered). Parses `src`, locates the postfix
/// chain straddling the cursor, and walks the segments left of it. Returns
/// `None` when the receiver can't be typed (the caller then offers nothing).
#[must_use]
pub fn owner_at_dot(
    ws: &Workspace,
    from_fqn: &str,
    src: &str,
    dot: u32,
) -> Option<(String, String)> {
    let module = pseudoscript_syntax::parse(src).ast;
    module.items.iter().find_map(|item| match item {
        ast::Item::Decl(decl) => decl_owner_at(ws, from_fqn, decl, dot),
        ast::Item::Feature(_) => None,
    })
}

fn decl_owner_at(
    ws: &Workspace,
    from_fqn: &str,
    decl: &ast::Decl,
    dot: u32,
) -> Option<(String, String)> {
    let (ast::DeclKind::Person(node)
    | ast::DeclKind::System(node)
    | ast::DeclKind::Container(node)
    | ast::DeclKind::Component(node)) = &decl.kind
    else {
        return None;
    };
    node.body.iter().flatten().find_map(|member| match member {
        ast::BodyMember::Callable(callable) => callable
            .body
            .as_ref()
            .and_then(|block| block_owner_at(ws, from_fqn, block, dot)),
        ast::BodyMember::Decl(inner) => decl_owner_at(ws, from_fqn, inner, dot),
    })
}

fn block_owner_at(
    ws: &Workspace,
    from_fqn: &str,
    block: &ast::Block,
    dot: u32,
) -> Option<(String, String)> {
    block.stmts.iter().find_map(|stmt| match &stmt.kind {
        ast::StmtKind::Assign { value, .. } => expr_owner_at(ws, from_fqn, value, dot),
        ast::StmtKind::Return(Some(e)) | ast::StmtKind::Expr(e) => {
            expr_owner_at(ws, from_fqn, e, dot)
        }
        ast::StmtKind::Return(None) => None,
        ast::StmtKind::If {
            cond,
            then_block,
            else_block,
        } => expr_owner_at(ws, from_fqn, cond, dot)
            .or_else(|| block_owner_at(ws, from_fqn, then_block, dot))
            .or_else(|| {
                else_block
                    .as_ref()
                    .and_then(|b| block_owner_at(ws, from_fqn, b, dot))
            }),
        ast::StmtKind::For { iter, body, .. } => expr_owner_at(ws, from_fqn, iter, dot)
            .or_else(|| block_owner_at(ws, from_fqn, body, dot)),
        ast::StmtKind::While { cond, body } => expr_owner_at(ws, from_fqn, cond, dot)
            .or_else(|| block_owner_at(ws, from_fqn, body, dot)),
    })
}

fn expr_owner_at(
    ws: &Workspace,
    from_fqn: &str,
    expr: &ast::Expr,
    dot: u32,
) -> Option<(String, String)> {
    match &expr.kind {
        ast::ExprKind::Postfix { base, segments } => {
            // The cursor may sit inside the base or a call argument's own chain —
            // descend there first; those are more specific than this chain.
            if let Some(owner) = expr_owner_at(ws, from_fqn, base, dot) {
                return Some(owner);
            }
            for seg in segments {
                for arg in seg.call_args.iter().flatten() {
                    if let Some(owner) = expr_owner_at(ws, from_fqn, arg, dot) {
                        return Some(owner);
                    }
                }
            }
            // The receiver is the base plus the segments whose name begins left
            // of the cursor; the trailing `.` sits past it, before the next name.
            let k = segments
                .iter()
                .take_while(|s| s.name.span.start < dot)
                .count();
            let recv_end = if k == 0 {
                base.span.end
            } else {
                segments[k - 1].span.end
            };
            let next_start = segments.get(k).map(|s| s.name.span.start);
            if recv_end <= dot && next_start.is_none_or(|ns| dot < ns) && dot <= expr.span.end {
                let mut owner = base_owner(ws, from_fqn, base)?;
                for seg in &segments[..k] {
                    let ty = ws
                        .module_model(&owner.0)?
                        .members(&owner.1)
                        .iter()
                        .find(|m| m.name == seg.name.name)?
                        .ty
                        .clone();
                    owner = type_owner(ws, from_fqn, &ty)?;
                }
                return Some(owner);
            }
            None
        }
        ast::ExprKind::Paren(inner) | ast::ExprKind::Unary { expr: inner, .. } => {
            expr_owner_at(ws, from_fqn, inner, dot)
        }
        ast::ExprKind::Marker {
            payload: Some(p), ..
        } => expr_owner_at(ws, from_fqn, p, dot),
        ast::ExprKind::From { source, .. } => source
            .sources()
            .iter()
            .find_map(|s| expr_owner_at(ws, from_fqn, s, dot)),
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

    // Mirrors the `syntax` module: `tokens` is annotated `Token[]`, reached
    // through the container-qualified `Syntax::Lexer`.
    const SYNTAX: &str = "//! syntax\n\n\
        public data Token { kind: string }\n\n\
        public component Lexer for Syntax {\n  tokenize(text: string): Token[];\n}\n\n\
        public component Parser for Syntax {\n  \
        public parse(text: string): string {\n    \
        tokens = Token[] from Syntax::Lexer.tokenize(text)\n    return tokens.kind\n  }\n}\n";

    #[test]
    fn local_type_reads_the_annotation() {
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
    fn owner_at_dot_types_a_binding_receiver() {
        // `tokens` is bound to `Token[]`; the `.` in `tokens.kind` resolves to
        // the `Token` owner so its members can be completed.
        let workspace = ws(&[("syntax", SYNTAX)]);
        let dot = (SYNTAX.find("tokens.kind").unwrap() + "tokens".len()) as u32;
        let owner = owner_at_dot(&workspace, "syntax", SYNTAX, dot).expect("typed receiver");
        assert_eq!(owner, ("syntax".to_owned(), "Token".to_owned()));
    }

    #[test]
    fn local_types_read_literal_and_from_annotations() {
        let src = "//! m\n\nsystem S {\n  go() {\n    n: number = 42\n    p: Parsed = Parsed from { n }\n  }\n}\n\npublic data Parsed { x: number }\n";
        let workspace = ws(&[("m", src)]);
        let locals = local_types(&workspace, "m");
        assert_eq!(locals.iter().find(|l| l.name == "n").unwrap().ty, "number");
        assert_eq!(locals.iter().find(|l| l.name == "p").unwrap().ty, "Parsed");
    }
}
