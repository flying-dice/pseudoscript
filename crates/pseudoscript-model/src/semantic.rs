//! AST-aware semantic tokens — the shared engine behind the LSP
//! (`pseudoscript-lsp`) and the web IDE (`pseudoscript-ide`).
//!
//! Two passes feed one sorted, non-overlapping token list in absolute byte
//! offsets:
//!
//! - **Token pass** ([`tokenize`]) colours the leaves the tree does not expose
//!   as convenient spans: keywords, doc comments, and string/number literals.
//! - **AST pass** ([`parse`]) colours identifiers by their *role* — a `system`
//!   name is a namespace, a `data` name a class, a callable a method, a
//!   parameter a parameter, a `.f()` step a method call — and a whole `#[…]`
//!   macro invocation as one decorator span. A later token overlapping an
//!   earlier one is dropped, so the macro span subsumes its arguments.
//!
//! Offsets are bytes, so the engine is adapter-neutral: the LSP delta-encodes to
//! `lsp_types`, the IDE serialises to JSON and decorates ranges.

use pseudoscript_syntax::{Span, TokenKind, Trivia, ast, lex, parse, tokenize};
use serde::Serialize;

/// A semantic token's role. Names follow the LSP standard token types so the
/// LSP adapter is a one-to-one mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SemKind {
    Namespace,
    Type,
    Class,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Method,
    Keyword,
    Comment,
    String,
    Number,
    Decorator,
}

/// One coloured span in absolute byte offsets, with the `declaration` modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SemToken {
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
    /// What the token denotes.
    pub kind: SemKind,
    /// Whether this is the declaration site of the symbol.
    pub declaration: bool,
}

/// Computes the semantic tokens for `src`, sorted by start offset and free of
/// overlaps and zero-width spans (a later token overlapping an earlier one is
/// dropped, matching the token-then-AST precedence).
#[must_use]
pub fn semantic_tokens(src: &str) -> Vec<SemToken> {
    let mut raws = Vec::new();
    token_pass(src, &mut raws);
    comment_pass(src, &mut raws);
    ast_pass(&parse(src).ast, &mut raws);
    raws.sort_by_key(|t| t.start);

    let mut out: Vec<SemToken> = Vec::with_capacity(raws.len());
    let mut last_end = 0;
    for raw in raws {
        if raw.start < last_end || raw.end <= raw.start {
            continue;
        }
        last_end = raw.end;
        out.push(raw);
    }
    out
}

/// Colours keywords, doc comments, and string/number literals.
fn token_pass(src: &str, out: &mut Vec<SemToken>) {
    for token in tokenize(src) {
        let kind = match token.kind {
            k if is_keyword(k) => SemKind::Keyword,
            TokenKind::Doc | TokenKind::InnerDoc | TokenKind::Tag => SemKind::Comment,
            TokenKind::String => SemKind::String,
            TokenKind::Number => SemKind::Number,
            // `#[…]` is coloured whole as a decorator by the AST pass, so the
            // opener is not coloured here (it would pre-empt the full span).
            _ => continue,
        };
        push(out, token.span, kind, false);
    }
}

/// Colours `//` line and `/* */` block comments. The lexer keeps these as
/// trivia (no token), so `token_pass` never sees them; without this pass only
/// `///` doc comments would be coloured. Blank-line trivia carries no glyph and
/// is skipped.
fn comment_pass(src: &str, out: &mut Vec<SemToken>) {
    for entry in lex(src).trivia {
        if matches!(
            entry.trivia,
            Trivia::LineComment(_) | Trivia::BlockComment(_)
        ) {
            push(out, entry.span, SemKind::Comment, false);
        }
    }
}

/// Whether `kind` is one of the reserved keywords (§2.3) — every `Kw*` token,
/// including the `Ok`/`Err`/`Some`/`None` constructors and the `feature`/
/// given/when/then/and/but BDD words. Colouring them all here means the AST pass
/// need not re-colour any keyword.
fn is_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwSystem
            | TokenKind::KwContainer
            | TokenKind::KwComponent
            | TokenKind::KwPerson
            | TokenKind::KwData
            | TokenKind::KwFor
            | TokenKind::KwFrom
            | TokenKind::KwPublic
            | TokenKind::KwSelf
            | TokenKind::KwReturn
            | TokenKind::KwOk
            | TokenKind::KwErr
            | TokenKind::KwSome
            | TokenKind::KwNone
            | TokenKind::KwIf
            | TokenKind::KwElse
            | TokenKind::KwWhile
            | TokenKind::KwIn
            | TokenKind::KwTrue
            | TokenKind::KwFalse
            | TokenKind::KwFeature
            | TokenKind::KwGiven
            | TokenKind::KwWhen
            | TokenKind::KwThen
            | TokenKind::KwAnd
            | TokenKind::KwBut
    )
}

/// Colours identifiers by their declared role across the whole module.
fn ast_pass(module: &ast::Module, out: &mut Vec<SemToken>) {
    for item in &module.items {
        match item {
            ast::Item::Decl(decl) => decl_tokens(decl, out),
            ast::Item::Feature(feature) => {
                push(out, feature.name.span, SemKind::Namespace, true);
                namespace_path(&feature.target, out);
            }
        }
    }
}

/// Colours a declaration: its macros, then its structural payload.
fn decl_tokens(decl: &ast::Decl, out: &mut Vec<SemToken>) {
    for mac in &decl.macros {
        macro_tokens(mac, out);
    }
    match &decl.kind {
        ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node) => node_tokens(node, out),
        ast::DeclKind::Data(data) => data_tokens(data, out),
        // The constant name is a value name; its literal is coloured by the
        // token pass (§3.6).
        ast::DeclKind::Constant(constant) => {
            push(out, constant.name.span, SemKind::Variable, true);
        }
    }
}

/// Colours a node: its name (namespace, declared), its `for` parent path, and
/// its disclosed body members.
fn node_tokens(node: &ast::Node, out: &mut Vec<SemToken>) {
    push(out, node.name.span, SemKind::Namespace, true);
    if let Some(parent) = &node.parent {
        namespace_path(parent, out);
    }
    for member in node.body.iter().flatten() {
        match member {
            ast::BodyMember::Callable(callable) => callable_tokens(callable, out),
            ast::BodyMember::Decl(decl) => decl_tokens(decl, out),
        }
    }
}

/// Colours a `data` declaration: its name (class, declared) and its fields or
/// variants.
fn data_tokens(data: &ast::Data, out: &mut Vec<SemToken>) {
    push(out, data.name.span, SemKind::Class, true);
    match &data.body {
        ast::DataBody::Record(fields) => fields_tokens(fields, out),
        ast::DataBody::Union(variants) => {
            for variant in variants {
                push(out, variant.name.span, SemKind::EnumMember, true);
                if let Some(fields) = &variant.record {
                    fields_tokens(fields, out);
                }
            }
        }
        ast::DataBody::BlackBox => {}
    }
}

/// Colours record fields: each name (property, declared) and its type.
fn fields_tokens(fields: &[ast::Field], out: &mut Vec<SemToken>) {
    for field in fields {
        push(out, field.name.span, SemKind::Property, true);
        type_tokens(&field.ty, out);
    }
}

/// Colours a callable: macros, name (method, declared), parameters, return
/// type, and body.
fn callable_tokens(callable: &ast::Callable, out: &mut Vec<SemToken>) {
    for mac in &callable.macros {
        macro_tokens(mac, out);
    }
    push(out, callable.name.span, SemKind::Method, true);
    for param in &callable.params {
        push(out, param.name.span, SemKind::Parameter, true);
        type_tokens(&param.ty, out);
    }
    if let Some(ret) = &callable.return_ty {
        type_tokens(ret, out);
    }
    if let Some(block) = &callable.body {
        block_tokens(block, out);
    }
}

/// Colours every statement in a block.
fn block_tokens(block: &ast::Block, out: &mut Vec<SemToken>) {
    for stmt in &block.stmts {
        stmt_tokens(stmt, out);
    }
}

/// Colours one statement and its sub-expressions.
fn stmt_tokens(stmt: &ast::Stmt, out: &mut Vec<SemToken>) {
    match &stmt.kind {
        ast::StmtKind::Assign { name, value } => {
            push(out, name.span, SemKind::Variable, true);
            // A binding states its type through `from` (ADR-035); the type is
            // coloured inside the `from` value.
            expr_tokens(value, out);
        }
        ast::StmtKind::Return(value) => {
            if let Some(value) = value {
                expr_tokens(value, out);
            }
        }
        ast::StmtKind::If {
            cond,
            then_block,
            else_block,
        } => {
            expr_tokens(cond, out);
            block_tokens(then_block, out);
            if let Some(block) = else_block {
                block_tokens(block, out);
            }
        }
        ast::StmtKind::For {
            binding,
            iter,
            body,
        } => {
            push(out, binding.span, SemKind::Variable, true);
            expr_tokens(iter, out);
            block_tokens(body, out);
        }
        ast::StmtKind::While { cond, body } => {
            expr_tokens(cond, out);
            block_tokens(body, out);
        }
        ast::StmtKind::Expr(expr) => expr_tokens(expr, out),
    }
}

/// Colours an expression: references, calls, field access, and `from` types.
fn expr_tokens(expr: &ast::Expr, out: &mut Vec<SemToken>) {
    match &expr.kind {
        ast::ExprKind::Marker { payload, .. } => {
            if let Some(payload) = payload {
                expr_tokens(payload, out);
            }
        }
        ast::ExprKind::From { ty, source } => {
            type_tokens(ty, out);
            for src in source.sources() {
                expr_tokens(src, out);
            }
        }
        ast::ExprKind::Postfix { base, segments } => {
            expr_tokens(base, out);
            for seg in segments {
                let kind = if seg.call_args.is_some() {
                    SemKind::Method
                } else {
                    SemKind::Property
                };
                push(out, seg.name.span, kind, false);
                for arg in seg.call_args.iter().flatten() {
                    expr_tokens(arg, out);
                }
            }
        }
        ast::ExprKind::Ref(ast::Ref::Path(path)) => ref_path(path, out),
        // `self` is a keyword and literals are coloured by the token pass.
        ast::ExprKind::Ref(ast::Ref::SelfNode(_)) | ast::ExprKind::Literal(_) => {}
        ast::ExprKind::Unary { expr, .. } => expr_tokens(expr, out),
        ast::ExprKind::Binary { left, right, .. } => {
            expr_tokens(left, out);
            expr_tokens(right, out);
        }
        ast::ExprKind::Paren(inner) => expr_tokens(inner, out),
    }
}

/// Colours a type: the base path (qualifiers as namespaces, the name as a type)
/// and any generic arguments.
fn type_tokens(ty: &ast::Type, out: &mut Vec<SemToken>) {
    type_path(&ty.name, out);
    for generic in &ty.generics {
        type_tokens(generic, out);
    }
}

/// Colours the whole macro invocation (`#[name(args)]`) as one decorator span,
/// so it reads as a cohesive unit rather than a name plus typed arguments.
fn macro_tokens(mac: &ast::Macro, out: &mut Vec<SemToken>) {
    push(out, mac.span, SemKind::Decorator, false);
}

/// Colours a value-reference path: trailing name as a variable, qualifiers as
/// namespaces.
fn ref_path(path: &ast::Path, out: &mut Vec<SemToken>) {
    split_path(path, SemKind::Variable, out);
}

/// Colours a type path: trailing name as a type, qualifiers as namespaces.
fn type_path(path: &ast::Path, out: &mut Vec<SemToken>) {
    split_path(path, SemKind::Type, out);
}

/// Colours every segment of a path as a namespace (parent / alias target).
fn namespace_path(path: &ast::Path, out: &mut Vec<SemToken>) {
    for segment in &path.segments {
        push(out, segment.span, SemKind::Namespace, false);
    }
}

/// Colours a path's last segment as `last`, every qualifier as a namespace.
fn split_path(path: &ast::Path, last: SemKind, out: &mut Vec<SemToken>) {
    let Some((name, qualifiers)) = path.segments.split_last() else {
        return;
    };
    for segment in qualifiers {
        push(out, segment.span, SemKind::Namespace, false);
    }
    push(out, name.span, last, false);
}

/// Records one coloured span.
fn push(out: &mut Vec<SemToken>, span: Span, kind: SemKind, declaration: bool) {
    out.push(SemToken {
        start: span.start,
        end: span.end,
        kind,
        declaration,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The token starting at the first occurrence of `needle`.
    fn at<'a>(tokens: &'a [SemToken], src: &str, needle: &str) -> &'a SemToken {
        let start = src.find(needle).expect("substring present") as u32;
        tokens
            .iter()
            .find(|t| t.start == start)
            .unwrap_or_else(|| panic!("no token at {needle:?}"))
    }

    #[test]
    fn line_and_block_comments_are_coloured() {
        // `//` and `/* */` are trivia, not tokens — they still colour as comments.
        let src = "//! m\n\n// a line note\nsystem S { /* inline */ }\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "// a line note").kind, SemKind::Comment);
        assert_eq!(at(&tokens, src, "/* inline */").kind, SemKind::Comment);
        // the `//!` module doc still colours too
        assert_eq!(at(&tokens, src, "//! m").kind, SemKind::Comment);
    }

    #[test]
    fn feature_and_step_keywords_are_coloured() {
        let src = "//! m\n\npublic system S;\n\nfeature Open for S {\n  given \"a user\"\n  when \"they act\"\n  then \"it works\"\n}\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "feature").kind, SemKind::Keyword);
        assert_eq!(at(&tokens, src, "given").kind, SemKind::Keyword);
        assert_eq!(at(&tokens, src, "when").kind, SemKind::Keyword);
        assert_eq!(at(&tokens, src, "then").kind, SemKind::Keyword);
        // the feature name is still a declared namespace
        assert_eq!(at(&tokens, src, "Open").kind, SemKind::Namespace);
    }

    #[test]
    fn option_markers_are_coloured() {
        let src = "//! m\n\nsystem S {\n  f(): void {\n    x: Option<number> = Some(1)\n    y: Option<number> = None\n  }\n}\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "Some").kind, SemKind::Keyword);
        assert_eq!(at(&tokens, src, "None").kind, SemKind::Keyword);
    }

    #[test]
    fn node_name_is_a_declared_namespace() {
        let src = "//! m\n\npublic system Banking;\n";
        let tokens = semantic_tokens(src);
        let token = at(&tokens, src, "Banking");
        assert_eq!(token.kind, SemKind::Namespace);
        assert!(token.declaration);
        assert_eq!(token.end - token.start, 7);
    }

    #[test]
    fn keyword_and_data_name_colours() {
        let src = "//! m\n\ndata Account { id: uuid }\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "data").kind, SemKind::Keyword);
        let account = at(&tokens, src, "Account");
        assert_eq!(account.kind, SemKind::Class);
        assert!(account.declaration);
        assert_eq!(at(&tokens, src, "id").kind, SemKind::Property);
        assert_eq!(at(&tokens, src, "uuid").kind, SemKind::Type);
    }

    #[test]
    fn callable_param_type_and_calls() {
        let src = "//! m\n\nsystem S {\n  run(name: string): uuid {\n    return self.alloc(name)\n  }\n}\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "run").kind, SemKind::Method);
        assert_eq!(at(&tokens, src, "name").kind, SemKind::Parameter);
        assert_eq!(at(&tokens, src, "string").kind, SemKind::Type);
        assert_eq!(at(&tokens, src, "self").kind, SemKind::Keyword);
        assert_eq!(at(&tokens, src, "alloc").kind, SemKind::Method);
    }

    #[test]
    fn member_access_vs_call() {
        let src = "//! m\n\nsystem S {\n  go() {\n    Repo.run(x)\n  }\n}\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "run").kind, SemKind::Method);
    }

    #[test]
    fn string_literal_and_keyword() {
        let src = "//! m\n\nsystem S {\n  f() {\n    return Err(\"boom\")\n  }\n}\n";
        let tokens = semantic_tokens(src);
        assert_eq!(at(&tokens, src, "\"boom\"").kind, SemKind::String);
        assert_eq!(at(&tokens, src, "Err").kind, SemKind::Keyword);
    }

    #[test]
    fn macro_invocation_is_one_decorator_span() {
        let src = "//! m\n\n#[onevent(a::B)]\nsystem S;\n";
        let tokens = semantic_tokens(src);
        let dec = at(&tokens, src, "#[onevent");
        assert_eq!(dec.kind, SemKind::Decorator);
        // covers the whole `#[…]`, subsuming the name and the path argument
        let close = (src.find(']').unwrap() + 1) as u32;
        assert!(dec.end >= close, "{dec:?}");
        assert!(
            !tokens
                .iter()
                .any(|t| t.start > dec.start && t.start < close),
            "args must be subsumed: {tokens:?}"
        );
    }

    #[test]
    fn tokens_are_sorted_and_non_overlapping() {
        let src = "//! m\n\ndata Account { id: uuid }\n";
        let tokens = semantic_tokens(src);
        let mut last_end = 0;
        for t in &tokens {
            assert!(t.start >= last_end, "overlap at {t:?}");
            assert!(t.end > t.start, "zero-width at {t:?}");
            last_end = t.end;
        }
    }
}
