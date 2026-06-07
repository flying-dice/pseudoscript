//! Token kinds and the [`Token`] type produced by the lexer.

use serde::{Deserialize, Serialize};

use crate::span::Span;

/// Every lexical token class in `PseudoScript` (`LANG.md` §2).
///
/// The canonical `KIND` string used by the conformance goldens is
/// [`TokenKind::name`] (e.g. `KW_SYSTEM`). Primitive type names
/// (`number`/`string`/…) and `Result` are **not** keywords — they lex as
/// [`TokenKind::Ident`] and are classified in type position by the model crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenKind {
    // Keywords (§2.3)
    KwSystem,
    KwContainer,
    KwComponent,
    KwPerson,
    KwData,
    KwFor,
    KwFrom,
    KwPublic,
    KwSelf,
    KwReturn,
    KwOk,
    KwErr,
    KwSome,
    KwNone,
    KwIf,
    KwElse,
    KwWhile,
    KwIn,
    KwTrue,
    KwFalse,
    KwFeature,
    KwGiven,
    KwWhen,
    KwThen,
    KwAnd,
    KwBut,

    /// Identifier (greedily matched; primitives and `Result` lex as this).
    Ident,

    // Punctuation & operators
    ColonColon,
    Dot,
    Colon,
    Semi,
    Comma,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Eq,
    Pipe,
    Question,
    LAngle,
    RAngle,
    Bang,

    // Literals
    /// Double-quoted string; the lexeme includes the quotes.
    String,
    /// Digit run, with an optional decimal part.
    Number,

    // Annotations (§2.1, §2.4)
    /// `///` doc text (marker and surrounding horizontal whitespace stripped).
    Doc,
    /// `//!` inner-doc text (same stripping).
    InnerDoc,
    /// `#name` tag on a `///` line; the lexeme includes the `#`.
    Tag,
    /// `#[`, opening a macro.
    HashLBracket,
}

impl TokenKind {
    /// The canonical uppercase `KIND` string used by the conformance goldens.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            TokenKind::KwSystem => "KW_SYSTEM",
            TokenKind::KwContainer => "KW_CONTAINER",
            TokenKind::KwComponent => "KW_COMPONENT",
            TokenKind::KwPerson => "KW_PERSON",
            TokenKind::KwData => "KW_DATA",
            TokenKind::KwFor => "KW_FOR",
            TokenKind::KwFrom => "KW_FROM",
            TokenKind::KwPublic => "KW_PUBLIC",
            TokenKind::KwSelf => "KW_SELF",
            TokenKind::KwReturn => "KW_RETURN",
            TokenKind::KwOk => "KW_OK",
            TokenKind::KwErr => "KW_ERR",
            TokenKind::KwSome => "KW_SOME",
            TokenKind::KwNone => "KW_NONE",
            TokenKind::KwIf => "KW_IF",
            TokenKind::KwElse => "KW_ELSE",
            TokenKind::KwWhile => "KW_WHILE",
            TokenKind::KwIn => "KW_IN",
            TokenKind::KwTrue => "KW_TRUE",
            TokenKind::KwFalse => "KW_FALSE",
            TokenKind::KwFeature => "KW_FEATURE",
            TokenKind::KwGiven => "KW_GIVEN",
            TokenKind::KwWhen => "KW_WHEN",
            TokenKind::KwThen => "KW_THEN",
            TokenKind::KwAnd => "KW_AND",
            TokenKind::KwBut => "KW_BUT",
            TokenKind::Ident => "IDENT",
            TokenKind::ColonColon => "COLONCOLON",
            TokenKind::Dot => "DOT",
            TokenKind::Colon => "COLON",
            TokenKind::Semi => "SEMI",
            TokenKind::Comma => "COMMA",
            TokenKind::LBrace => "LBRACE",
            TokenKind::RBrace => "RBRACE",
            TokenKind::LParen => "LPAREN",
            TokenKind::RParen => "RPAREN",
            TokenKind::LBracket => "LBRACKET",
            TokenKind::RBracket => "RBRACKET",
            TokenKind::Eq => "EQ",
            TokenKind::Pipe => "PIPE",
            TokenKind::Question => "QUESTION",
            TokenKind::LAngle => "LANGLE",
            TokenKind::RAngle => "RANGLE",
            TokenKind::Bang => "BANG",
            TokenKind::String => "STRING",
            TokenKind::Number => "NUMBER",
            TokenKind::Doc => "DOC",
            TokenKind::InnerDoc => "INNER_DOC",
            TokenKind::Tag => "TAG",
            TokenKind::HashLBracket => "HASH_LBRACKET",
        }
    }

    /// The reserved keyword spellings (§2.3), in declaration order. Every entry
    /// is recognised by [`TokenKind::keyword`]; a test pins them in sync.
    pub const KEYWORDS: [&str; 26] = [
        "system",
        "container",
        "component",
        "person",
        "data",
        "for",
        "from",
        "public",
        "self",
        "return",
        "Ok",
        "Err",
        "Some",
        "None",
        "if",
        "else",
        "while",
        "in",
        "true",
        "false",
        "feature",
        "given",
        "when",
        "then",
        "and",
        "but",
    ];

    /// The primitive type names (§3.1). `Result` is reserved (§6) but not a
    /// primitive, so it is not listed here.
    pub const PRIMITIVE_TYPES: [&str; 6] = ["number", "string", "bool", "datetime", "uuid", "void"];

    /// If `ident` is a keyword, the matching keyword kind; otherwise `None`.
    #[must_use]
    pub fn keyword(ident: &str) -> Option<TokenKind> {
        let kind = match ident {
            "system" => TokenKind::KwSystem,
            "container" => TokenKind::KwContainer,
            "component" => TokenKind::KwComponent,
            "person" => TokenKind::KwPerson,
            "data" => TokenKind::KwData,
            "for" => TokenKind::KwFor,
            "from" => TokenKind::KwFrom,
            "public" => TokenKind::KwPublic,
            "self" => TokenKind::KwSelf,
            "return" => TokenKind::KwReturn,
            "Ok" => TokenKind::KwOk,
            "Err" => TokenKind::KwErr,
            "Some" => TokenKind::KwSome,
            "None" => TokenKind::KwNone,
            "if" => TokenKind::KwIf,
            "else" => TokenKind::KwElse,
            "while" => TokenKind::KwWhile,
            "in" => TokenKind::KwIn,
            "true" => TokenKind::KwTrue,
            "false" => TokenKind::KwFalse,
            "feature" => TokenKind::KwFeature,
            "given" => TokenKind::KwGiven,
            "when" => TokenKind::KwWhen,
            "then" => TokenKind::KwThen,
            "and" => TokenKind::KwAnd,
            "but" => TokenKind::KwBut,
            _ => return None,
        };
        Some(kind)
    }
}

/// A single lexed token: its kind, source span, and rendered lexeme.
///
/// For most tokens `text` is the raw source slice. For [`TokenKind::Doc`] and
/// [`TokenKind::InnerDoc`] it is the doc text with the marker and surrounding
/// horizontal whitespace stripped; for [`TokenKind::Tag`] it includes the `#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The token class.
    pub kind: TokenKind,
    /// Source range of the token (the full marker for doc/tag tokens).
    pub span: Span,
    /// The rendered lexeme (see type docs for doc/tag specifics).
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::TokenKind;

    #[test]
    fn keywords_const_matches_recognizer() {
        // Every advertised keyword lexes as a keyword, and the recognizer
        // accepts nothing outside the list (primitives stay identifiers).
        for kw in TokenKind::KEYWORDS {
            assert!(TokenKind::keyword(kw).is_some(), "missing keyword: {kw}");
        }
        for prim in TokenKind::PRIMITIVE_TYPES {
            assert!(
                TokenKind::keyword(prim).is_none(),
                "primitive must lex as ident: {prim}"
            );
        }
    }
}
