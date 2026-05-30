//! AST-aware semantic tokens (LSP `textDocument/semanticTokens`).
//!
//! Two passes feed one sorted, non-overlapping, delta-encoded token list:
//!
//! - **Token pass** ([`tokenize`]) colours the leaves the tree does not expose
//!   as convenient spans: keywords, doc comments, string/number literals, and
//!   the `#[` macro opener. These kinds are never identifiers.
//! - **AST pass** ([`parse`]) colours identifiers by their *role* — a `system`
//!   name is a namespace, a `data` name a class, a callable a method, a
//!   parameter a parameter, a `.f()` step a method call. Identifiers are never
//!   touched by the token pass, so the two sets cannot overlap.
//!
//! The shared [`legend`] keeps the server capability and the encoding in step:
//! [`Sem`] discriminants index it directly.

use pseudoscript_syntax::{LineIndex, Span, TokenKind, ast, parse, tokenize};
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use crate::convert::offset_to_position;

/// A semantic token type. The discriminant is its index into [`legend`]'s
/// `token_types`, so the two MUST stay in the same order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sem {
    Namespace = 0,
    Type,
    Class,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Method,
    Keyword,
    Comment,
    Str,
    Number,
    Decorator,
}

impl Sem {
    /// The token type's index into the legend.
    fn index(self) -> u32 {
        self as u32
    }
}

/// The `declaration` modifier bit (bit 0 of the legend's modifier list).
const MOD_DECLARATION: u32 = 1 << 0;

/// The token types this server emits, in legend order (see [`Sem`]).
fn token_types() -> Vec<SemanticTokenType> {
    vec![
        SemanticTokenType::NAMESPACE,
        SemanticTokenType::TYPE,
        SemanticTokenType::CLASS,
        SemanticTokenType::PARAMETER,
        SemanticTokenType::VARIABLE,
        SemanticTokenType::PROPERTY,
        SemanticTokenType::ENUM_MEMBER,
        SemanticTokenType::METHOD,
        SemanticTokenType::KEYWORD,
        SemanticTokenType::COMMENT,
        SemanticTokenType::STRING,
        SemanticTokenType::NUMBER,
        SemanticTokenType::DECORATOR,
    ]
}

/// The legend the server advertises and the encoder indexes into.
#[must_use]
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: token_types(),
        token_modifiers: vec![SemanticTokenModifier::DECLARATION],
    }
}

/// A coloured span before delta-encoding.
struct Raw {
    span: Span,
    ty: Sem,
    mods: u32,
}

/// Computes the semantic tokens for `src` as a full-document set.
#[must_use]
pub fn semantic_tokens(src: &str) -> SemanticTokens {
    let mut raws = Vec::new();
    token_pass(src, &mut raws);
    ast_pass(&parse(src).ast, &mut raws);
    encode(src, &raws)
}

/// Colours keywords, doc comments, literals, and the `#[` macro opener.
fn token_pass(src: &str, out: &mut Vec<Raw>) {
    for token in tokenize(src) {
        let ty = match token.kind {
            k if is_keyword(k) => Sem::Keyword,
            TokenKind::Doc | TokenKind::InnerDoc | TokenKind::Tag => Sem::Comment,
            TokenKind::String => Sem::Str,
            TokenKind::Number => Sem::Number,
            TokenKind::HashLBracket => Sem::Decorator,
            _ => continue,
        };
        out.push(Raw {
            span: token.span,
            ty,
            mods: 0,
        });
    }
}

/// Whether `kind` is one of the reserved keywords (§2.3). `self`, `Ok`, `Err`,
/// `true`, and `false` count, so the AST pass need not re-colour them.
fn is_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwSystem
            | TokenKind::KwContainer
            | TokenKind::KwComponent
            | TokenKind::KwPerson
            | TokenKind::KwData
            | TokenKind::KwFor
            | TokenKind::KwAlias
            | TokenKind::KwFrom
            | TokenKind::KwPublic
            | TokenKind::KwSelf
            | TokenKind::KwReturn
            | TokenKind::KwOk
            | TokenKind::KwErr
            | TokenKind::KwIf
            | TokenKind::KwElse
            | TokenKind::KwWhile
            | TokenKind::KwIn
            | TokenKind::KwTrue
            | TokenKind::KwFalse
    )
}

/// Colours identifiers by their declared role across the whole module.
fn ast_pass(module: &ast::Module, out: &mut Vec<Raw>) {
    for item in &module.items {
        match item {
            ast::Item::Alias(alias) => {
                push(out, alias.name.span, Sem::Namespace, MOD_DECLARATION);
                namespace_path(&alias.target, out);
            }
            ast::Item::Decl(decl) => decl_tokens(decl, out),
            ast::Item::Feature(feature) => {
                push(out, feature.name.span, Sem::Namespace, MOD_DECLARATION);
                namespace_path(&feature.target, out);
            }
        }
    }
}

/// Colours a declaration: its macros, then its structural payload.
fn decl_tokens(decl: &ast::Decl, out: &mut Vec<Raw>) {
    for mac in &decl.macros {
        macro_tokens(mac, out);
    }
    match &decl.kind {
        ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node) => node_tokens(node, out),
        ast::DeclKind::Data(data) => data_tokens(data, out),
    }
}

/// Colours a node: its name (namespace, declared), its `for` parent path, and
/// its disclosed body members.
fn node_tokens(node: &ast::Node, out: &mut Vec<Raw>) {
    push(out, node.name.span, Sem::Namespace, MOD_DECLARATION);
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
fn data_tokens(data: &ast::Data, out: &mut Vec<Raw>) {
    push(out, data.name.span, Sem::Class, MOD_DECLARATION);
    match &data.body {
        ast::DataBody::Record(fields) => fields_tokens(fields, out),
        ast::DataBody::Union(variants) => {
            for variant in variants {
                push(out, variant.name.span, Sem::EnumMember, MOD_DECLARATION);
                if let Some(fields) = &variant.record {
                    fields_tokens(fields, out);
                }
            }
        }
        ast::DataBody::BlackBox => {}
    }
}

/// Colours record fields: each name (property, declared) and its type.
fn fields_tokens(fields: &[ast::Field], out: &mut Vec<Raw>) {
    for field in fields {
        push(out, field.name.span, Sem::Property, MOD_DECLARATION);
        type_tokens(&field.ty, out);
    }
}

/// Colours a callable: macros, name (method, declared), parameters, return
/// type, and body.
fn callable_tokens(callable: &ast::Callable, out: &mut Vec<Raw>) {
    for mac in &callable.macros {
        macro_tokens(mac, out);
    }
    push(out, callable.name.span, Sem::Method, MOD_DECLARATION);
    for param in &callable.params {
        push(out, param.name.span, Sem::Parameter, MOD_DECLARATION);
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
fn block_tokens(block: &ast::Block, out: &mut Vec<Raw>) {
    for stmt in &block.stmts {
        stmt_tokens(stmt, out);
    }
}

/// Colours one statement and its sub-expressions.
fn stmt_tokens(stmt: &ast::Stmt, out: &mut Vec<Raw>) {
    match &stmt.kind {
        ast::StmtKind::Assign { name, value } => {
            push(out, name.span, Sem::Variable, MOD_DECLARATION);
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
            push(out, binding.span, Sem::Variable, MOD_DECLARATION);
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
fn expr_tokens(expr: &ast::Expr, out: &mut Vec<Raw>) {
    match &expr.kind {
        ast::ExprKind::Marker { payload, .. } => {
            if let Some(payload) = payload {
                expr_tokens(payload, out);
            }
        }
        ast::ExprKind::From { ty, sources, .. } => {
            type_path(ty, out);
            for source in sources {
                expr_tokens(source, out);
            }
        }
        ast::ExprKind::Postfix { base, segments } => {
            expr_tokens(base, out);
            for seg in segments {
                let ty = if seg.call_args.is_some() {
                    Sem::Method
                } else {
                    Sem::Property
                };
                push(out, seg.name.span, ty, 0);
                for arg in seg.call_args.iter().flatten() {
                    expr_tokens(arg, out);
                }
            }
        }
        ast::ExprKind::Ref(ast::Ref::Path(path)) => ref_path(path, out),
        // `self` is a keyword and literals are coloured by the token pass.
        ast::ExprKind::Ref(ast::Ref::SelfNode(_)) | ast::ExprKind::Literal(_) => {}
        ast::ExprKind::Unary { expr, .. } => expr_tokens(expr, out),
        ast::ExprKind::Paren(inner) => expr_tokens(inner, out),
    }
}

/// Colours a type: the base path (qualifiers as namespaces, the name as a type)
/// and any generic arguments.
fn type_tokens(ty: &ast::Type, out: &mut Vec<Raw>) {
    type_path(&ty.name, out);
    for generic in &ty.generics {
        type_tokens(generic, out);
    }
}

/// Colours a macro's name path as a decorator; its path/literal arguments take
/// type colours (literals are already coloured by the token pass).
fn macro_tokens(mac: &ast::Macro, out: &mut Vec<Raw>) {
    for segment in &mac.name.segments {
        push(out, segment.span, Sem::Decorator, 0);
    }
    if let ast::MacroArgs::List(args) = &mac.args {
        for arg in args {
            match arg {
                ast::MacroArg::Path(path) => type_path(path, out),
                ast::MacroArg::Nested(nested) => macro_tokens(nested, out),
                ast::MacroArg::Literal(_) => {}
            }
        }
    }
}

/// Colours a value-reference path: trailing name as a variable, qualifiers as
/// namespaces.
fn ref_path(path: &ast::Path, out: &mut Vec<Raw>) {
    split_path(path, Sem::Variable, out);
}

/// Colours a type path: trailing name as a type, qualifiers as namespaces.
fn type_path(path: &ast::Path, out: &mut Vec<Raw>) {
    split_path(path, Sem::Type, out);
}

/// Colours every segment of a path as a namespace (parent / alias target).
fn namespace_path(path: &ast::Path, out: &mut Vec<Raw>) {
    for segment in &path.segments {
        push(out, segment.span, Sem::Namespace, 0);
    }
}

/// Colours a path's last segment as `last`, every qualifier as a namespace.
fn split_path(path: &ast::Path, last: Sem, out: &mut Vec<Raw>) {
    let Some((name, qualifiers)) = path.segments.split_last() else {
        return;
    };
    for segment in qualifiers {
        push(out, segment.span, Sem::Namespace, 0);
    }
    push(out, name.span, last, 0);
}

/// Records one coloured span.
fn push(out: &mut Vec<Raw>, span: Span, ty: Sem, mods: u32) {
    out.push(Raw { span, ty, mods });
}

/// Sorts the raw spans and delta-encodes them into LSP semantic tokens.
///
/// Tokens are ordered by start offset; any token overlapping its predecessor,
/// spanning more than one line, or of zero width is dropped (the protocol
/// requires single-line, non-overlapping tokens).
fn encode(src: &str, raws: &[Raw]) -> SemanticTokens {
    let index = LineIndex::new(src);
    let mut ordered: Vec<&Raw> = raws.iter().collect();
    ordered.sort_by_key(|raw| raw.span.start);

    let mut data = Vec::new();
    let mut prev_line = 0;
    let mut prev_start = 0;
    let mut last_end = 0;

    for raw in ordered {
        if raw.span.start < last_end {
            continue;
        }
        let start = offset_to_position(src, &index, raw.span.start);
        let end = offset_to_position(src, &index, raw.span.end);
        if end.line != start.line || end.character <= start.character {
            continue;
        }
        let delta_line = start.line - prev_line;
        let delta_start = if delta_line == 0 {
            start.character - prev_start
        } else {
            start.character
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: end.character - start.character,
            token_type: raw.ty.index(),
            token_modifiers_bitset: raw.mods,
        });
        prev_line = start.line;
        prev_start = start.character;
        last_end = raw.span.end;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A decoded token in absolute coordinates, for assertion convenience.
    #[derive(Debug, PartialEq, Eq)]
    struct Decoded {
        line: u32,
        start: u32,
        len: u32,
        ty: SemanticTokenType,
        declared: bool,
    }

    fn decode(src: &str) -> Vec<Decoded> {
        let types = token_types();
        let tokens = semantic_tokens(src);
        let mut line = 0;
        let mut start = 0;
        tokens
            .data
            .iter()
            .map(|t| {
                if t.delta_line == 0 {
                    start += t.delta_start;
                } else {
                    line += t.delta_line;
                    start = t.delta_start;
                }
                Decoded {
                    line,
                    start,
                    len: t.length,
                    ty: types[t.token_type as usize].clone(),
                    declared: t.token_modifiers_bitset & MOD_DECLARATION != 0,
                }
            })
            .collect()
    }

    /// Finds the decoded token starting at the first occurrence of `needle`.
    fn at<'a>(decoded: &'a [Decoded], src: &str, needle: &str) -> &'a Decoded {
        let offset = src.find(needle).expect("substring present") as u32;
        let line = src[..offset as usize].matches('\n').count() as u32;
        let line_start = src[..offset as usize].rfind('\n').map_or(0, |nl| nl + 1) as u32;
        let start = offset - line_start;
        decoded
            .iter()
            .find(|d| d.line == line && d.start == start)
            .unwrap_or_else(|| panic!("no token at {needle:?}"))
    }

    #[test]
    fn legend_indices_match_discriminants() {
        let types = token_types();
        assert_eq!(types[Sem::Namespace as usize], SemanticTokenType::NAMESPACE);
        assert_eq!(types[Sem::Decorator as usize], SemanticTokenType::DECORATOR);
        assert_eq!(types.len(), Sem::Decorator as usize + 1);
    }

    #[test]
    fn node_name_is_a_declared_namespace() {
        let src = "//! m\n\npublic system Banking;\n";
        let decoded = decode(src);
        let token = at(&decoded, src, "Banking");
        assert_eq!(token.ty, SemanticTokenType::NAMESPACE);
        assert!(token.declared);
        assert_eq!(token.len, 7);
    }

    #[test]
    fn keyword_and_data_name_colours() {
        let src = "//! m\n\ndata Account { id: uuid }\n";
        let decoded = decode(src);
        assert_eq!(at(&decoded, src, "data").ty, SemanticTokenType::KEYWORD);
        let account = at(&decoded, src, "Account");
        assert_eq!(account.ty, SemanticTokenType::CLASS);
        assert!(account.declared);
        assert_eq!(at(&decoded, src, "id").ty, SemanticTokenType::PROPERTY);
        assert_eq!(at(&decoded, src, "uuid").ty, SemanticTokenType::TYPE);
    }

    #[test]
    fn callable_param_type_and_calls() {
        let src = "//! m\n\nsystem S {\n  run(name: string): uuid {\n    return self.alloc(name)\n  }\n}\n";
        let decoded = decode(src);
        assert_eq!(at(&decoded, src, "run").ty, SemanticTokenType::METHOD);
        assert_eq!(at(&decoded, src, "name").ty, SemanticTokenType::PARAMETER);
        assert_eq!(at(&decoded, src, "string").ty, SemanticTokenType::TYPE);
        assert_eq!(at(&decoded, src, "self").ty, SemanticTokenType::KEYWORD);
        // the `.alloc(...)` call segment
        assert_eq!(at(&decoded, src, "alloc").ty, SemanticTokenType::METHOD);
    }

    #[test]
    fn member_call_is_a_method_token() {
        // `run` in `Repo.run(x)` is a call segment → coloured as a method.
        let src = "//! m\n\nsystem S {\n  go() {\n    Repo.run(x)\n  }\n}\n";
        let decoded = decode(src);
        assert_eq!(at(&decoded, src, "run").ty, SemanticTokenType::METHOD);
    }

    #[test]
    fn macro_name_is_a_decorator() {
        let src = "//! m\n\n#[diagram]\nsystem S;\n";
        let decoded = decode(src);
        assert_eq!(
            at(&decoded, src, "diagram").ty,
            SemanticTokenType::DECORATOR
        );
    }

    #[test]
    fn string_literal_is_a_string() {
        let src = "//! m\n\nsystem S {\n  f() {\n    return Err(\"boom\")\n  }\n}\n";
        let decoded = decode(src);
        assert_eq!(at(&decoded, src, "\"boom\"").ty, SemanticTokenType::STRING);
        assert_eq!(at(&decoded, src, "Err").ty, SemanticTokenType::KEYWORD);
    }
}
