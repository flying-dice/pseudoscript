//! Context-aware completion — the shared engine behind both the LSP
//! (`pseudoscript-lsp`) and the web IDE (`pseudoscript-wasm`).
//!
//! The completion context is read from the token immediately left of the caret:
//!
//! - after `.` → the base node's callables and fields;
//! - after `::` → the named module's (public) symbols;
//! - after `#[` → the built-in macros;
//! - after `:` or `<` (type position) → primitive types, `Result`, and every
//!   declared type;
//! - otherwise → keywords, this module's symbols, and its aliases.
//!
//! The caller filters the returned set against the prefix being typed, so the
//! full candidate list is offered. Positions are byte offsets, so the engine is
//! adapter-neutral: the LSP maps to `lsp_types`, the IDE serialises to JSON.

use pseudoscript_syntax::{Token, TokenKind, tokenize};
use serde::Serialize;

use crate::resolve::{enclosing_node, module_of};
use crate::{BUILTIN_MACROS, MemberKind, SymbolKind, Workspace};

/// What a completion candidate denotes — drives the icon each surface renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompletionKind {
    /// A callable member reached through `.`.
    Method,
    /// A record field reached through `.`.
    Field,
    /// A language keyword.
    Keyword,
    /// A built-in macro (`#[...]`).
    Macro,
    /// A primitive type or `Result`.
    Type,
    /// A `data` declaration.
    Class,
    /// A node declaration (system / container / component / person).
    Module,
    /// An alias.
    Reference,
}

/// One completion candidate: its insert text, what it is, and a one-line detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompletionItem {
    /// The text offered (and inserted).
    pub label: String,
    /// What the candidate denotes.
    pub kind: CompletionKind,
    /// A one-line detail (signature, fqn, or category).
    pub detail: String,
}

/// Completion candidates for byte `offset` in module `from_fqn`'s `src`.
#[must_use]
pub fn completion(ws: &Workspace, from_fqn: &str, src: &str, offset: u32) -> Vec<CompletionItem> {
    let tokens = tokenize(src);
    let trigger = governing_trigger(&tokens, offset);

    match trigger.map(|i| (i, tokens[i].kind)) {
        Some((i, TokenKind::Dot)) => member_items(ws, from_fqn, &tokens, i),
        Some((i, TokenKind::ColonColon)) => path_items(ws, from_fqn, &tokens, i),
        Some((_, TokenKind::HashLBracket)) => macro_items(),
        Some((_, TokenKind::Colon | TokenKind::LAngle)) => type_items(ws),
        _ => general_items(ws, from_fqn),
    }
}

/// Index of the token whose kind governs completion at `offset`.
///
/// The trigger is the rightmost token ending at or before the caret — except
/// for a partial identifier typed *under* the caret, which ends exactly at
/// `offset` (`span.end == offset`). That identifier is the prefix the caller
/// filters on, not the context, so its predecessor is the real trigger. A caret
/// strictly inside an identifier (`span.end > offset`) is already excluded by
/// the `<= offset` bound, so only the boundary case needs skipping.
fn governing_trigger(tokens: &[Token], offset: u32) -> Option<usize> {
    let last = tokens.iter().rposition(|t| t.span.end <= offset)?;
    if tokens[last].kind == TokenKind::Ident && tokens[last].span.end == offset {
        last.checked_sub(1)
    } else {
        Some(last)
    }
}

/// Callables and fields of the node named by the base before `tokens[dot]`.
fn member_items(
    ws: &Workspace,
    from_fqn: &str,
    tokens: &[Token],
    dot: usize,
) -> Vec<CompletionItem> {
    let Some((owner_module, owner_name)) = owner_before(ws, from_fqn, tokens, dot) else {
        return Vec::new();
    };
    let Some(entry) = ws.module(&owner_module) else {
        return Vec::new();
    };
    entry
        .model
        .members(&owner_name)
        .iter()
        .map(|m| {
            let kind = match m.kind {
                MemberKind::Callable => CompletionKind::Method,
                MemberKind::Field => CompletionKind::Field,
            };
            item(&m.name, kind, &m.detail)
        })
        .collect()
}

/// The `(module, node-name)` the base token before `tokens[dot]` denotes:
/// `self`'s enclosing node, or an in-scope node name.
fn owner_before(
    ws: &Workspace,
    from_fqn: &str,
    tokens: &[Token],
    dot: usize,
) -> Option<(String, String)> {
    let base = tokens.get(dot.checked_sub(1)?)?;
    match base.kind {
        TokenKind::KwSelf => {
            let node = enclosing_node(&ws.module(from_fqn)?.ast, base.span.start)?;
            Some((from_fqn.to_owned(), node))
        }
        TokenKind::Ident => {
            let symbol = ws.module(from_fqn)?.model.symbol(&base.text)?;
            Some((module_of(&symbol.fqn).to_owned(), symbol.name.clone()))
        }
        _ => None,
    }
}

/// Symbols of the module named by the `::` path ending at `tokens[ccolon]`.
/// A cross-module suggestion is offered only when `public` (§8.2).
fn path_items(
    ws: &Workspace,
    from_fqn: &str,
    tokens: &[Token],
    ccolon: usize,
) -> Vec<CompletionItem> {
    let prefix = module_prefix(tokens, ccolon);
    ws.symbols()
        .filter(|s| module_of(&s.fqn) == prefix && (prefix == from_fqn || s.is_public))
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn))
        .collect()
}

/// The `::`-joined module path written immediately before `tokens[ccolon]`.
fn module_prefix(tokens: &[Token], ccolon: usize) -> String {
    let mut segments = Vec::new();
    let mut cursor = ccolon.checked_sub(1);
    while let Some(i) = cursor {
        if tokens[i].kind != TokenKind::Ident {
            break;
        }
        segments.push(tokens[i].text.as_str());
        cursor = match i.checked_sub(1) {
            Some(j) if tokens[j].kind == TokenKind::ColonColon => i.checked_sub(2),
            _ => None,
        };
    }
    segments.reverse();
    segments.join("::")
}

/// The built-in macros (`#[...]`).
fn macro_items() -> Vec<CompletionItem> {
    BUILTIN_MACROS
        .iter()
        .map(|m| item(m, CompletionKind::Macro, "built-in macro"))
        .collect()
}

/// Primitive types, `Result`, and every declared node/data type.
fn type_items(ws: &Workspace) -> Vec<CompletionItem> {
    let primitives = TokenKind::PRIMITIVE_TYPES
        .iter()
        .map(|t| item(t, CompletionKind::Type, "primitive type"))
        .chain(std::iter::once(item(
            "Result",
            CompletionKind::Type,
            "built-in generic",
        )));
    let declared = ws
        .symbols()
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn));
    primitives.chain(declared).collect()
}

/// Keywords plus this module's own symbols and aliases.
fn general_items(ws: &Workspace, from_fqn: &str) -> Vec<CompletionItem> {
    let keywords = TokenKind::KEYWORDS
        .iter()
        .map(|k| item(k, CompletionKind::Keyword, "keyword"));
    let Some(entry) = ws.module(from_fqn) else {
        return keywords.collect();
    };
    let symbols = entry
        .model
        .symbols()
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn));
    let aliases = entry
        .model
        .aliases()
        .map(|(name, a)| item(name, CompletionKind::Reference, &a.target));
    keywords.chain(symbols).chain(aliases).collect()
}

/// The completion kind for a declared symbol.
fn symbol_kind(kind: SymbolKind) -> CompletionKind {
    match kind {
        SymbolKind::Data => CompletionKind::Class,
        _ => CompletionKind::Module,
    }
}

/// Builds a labelled completion item with a detail string.
fn item(label: &str, kind: CompletionKind, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        kind,
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_syntax::parse;

    fn workspace(modules: &[(&str, &str)]) -> Workspace {
        Workspace::build(
            modules
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
        )
    }

    /// Completion labels at byte `offset` in module `from`.
    fn labels_at(ws: &Workspace, from: &str, src: &str, offset: u32) -> Vec<String> {
        completion(ws, from, src, offset)
            .into_iter()
            .map(|c| c.label)
            .collect()
    }

    #[test]
    fn members_after_self_dot() {
        let src = "//! m\n\nsystem S {\n  run() {\n    self.\n  }\n  helper(x: number): uuid;\n}\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("self.").unwrap() + "self.".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"helper".to_owned()), "{labels:?}");
        assert!(labels.contains(&"run".to_owned()), "{labels:?}");
    }

    #[test]
    fn types_after_colon() {
        let src = "//! m\n\ndata D { x: }\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("x:").unwrap() + 2) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"number".to_owned()), "{labels:?}");
        assert!(labels.contains(&"Result".to_owned()), "{labels:?}");
        assert!(labels.contains(&"D".to_owned()), "{labels:?}");
    }

    #[test]
    fn macros_after_hash_bracket() {
        let src = "//! m\n\n#[\nsystem S;\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("#[").unwrap() + 2) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"http".to_owned()), "{labels:?}");
        assert!(labels.contains(&"onevent".to_owned()), "{labels:?}");
    }

    #[test]
    fn public_symbols_after_module_path() {
        let mods = [
            ("a", "//! a\n\npublic system Svc;\n\nsystem Hidden;\n"),
            ("b", "//! b\n\ncontainer C for a::\n"),
        ];
        let ws = workspace(&mods);
        let src = mods[1].1;
        let offset = (src.find("a::").unwrap() + 3) as u32;
        let labels = labels_at(&ws, "b", src, offset);
        assert!(labels.contains(&"Svc".to_owned()), "{labels:?}");
        assert!(!labels.contains(&"Hidden".to_owned()), "{labels:?}");
    }

    #[test]
    fn keywords_in_general_position() {
        let src = "//! m\n\n";
        let ws = workspace(&[("m", src)]);
        let labels = labels_at(&ws, "m", src, src.len() as u32);
        assert!(labels.contains(&"system".to_owned()), "{labels:?}");
        assert!(labels.contains(&"public".to_owned()), "{labels:?}");
    }

    // With a prefix typed, the caret sits at the end of a partial identifier.
    // Each narrowing context must stay scoped — the trigger before the prefix
    // governs — and must not leak the general keyword set.

    #[test]
    fn members_after_self_dot_with_prefix() {
        let src =
            "//! m\n\nsystem S {\n  run() {\n    self.he\n  }\n  helper(x: number): uuid;\n}\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("self.he").unwrap() + "self.he".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"helper".to_owned()), "{labels:?}");
        assert!(labels.contains(&"run".to_owned()), "{labels:?}");
        assert!(
            !labels.contains(&"system".to_owned()),
            "general scope leaked: {labels:?}"
        );
    }

    #[test]
    fn types_after_colon_with_prefix() {
        let src = "//! m\n\ndata D { x: numb }\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("numb").unwrap() + "numb".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"number".to_owned()), "{labels:?}");
        assert!(labels.contains(&"D".to_owned()), "{labels:?}");
        assert!(
            !labels.contains(&"system".to_owned()),
            "general scope leaked: {labels:?}"
        );
    }

    #[test]
    fn macros_after_hash_bracket_with_prefix() {
        let src = "//! m\n\n#[ht\nsystem S;\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("#[ht").unwrap() + "#[ht".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"http".to_owned()), "{labels:?}");
        assert!(
            !labels.contains(&"system".to_owned()),
            "general scope leaked: {labels:?}"
        );
    }

    #[test]
    fn public_symbols_after_module_path_with_prefix() {
        let mods = [
            ("a", "//! a\n\npublic system Svc;\n\nsystem Hidden;\n"),
            ("b", "//! b\n\ncontainer C for a::Sv\n"),
        ];
        let ws = workspace(&mods);
        let src = mods[1].1;
        let offset = (src.find("a::Sv").unwrap() + "a::Sv".len()) as u32;
        let labels = labels_at(&ws, "b", src, offset);
        assert!(labels.contains(&"Svc".to_owned()), "{labels:?}");
        assert!(!labels.contains(&"Hidden".to_owned()), "{labels:?}");
        assert!(
            !labels.contains(&"system".to_owned()),
            "general scope leaked: {labels:?}"
        );
    }
}
