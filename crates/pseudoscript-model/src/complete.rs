//! Context-aware completion — the shared engine behind both the LSP
//! (`pseudoscript-lsp`) and the web IDE (`pseudoscript-ide`).
//!
//! The completion context is read from the token immediately left of the caret:
//!
//! - after `.` → the base node's callables and fields;
//! - after `::` → the named module's (public) symbols;
//! - after `#[` → the built-in macros;
//! - after `:` or `<` (type position) → primitive types, `Result`, and every
//!   declared type;
//! - otherwise → keywords, this module's symbols, and the other workspace
//!   modules (to start a cross-module reference).
//!
//! The caller filters the returned set against the prefix being typed, so the
//! full candidate list is offered. Positions are byte offsets, so the engine is
//! adapter-neutral: the LSP maps to `lsp_types`, the IDE serialises to JSON.

use std::collections::BTreeSet;

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
    /// A `constant` declaration (§3.6).
    Constant,
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
#[tracing::instrument(level = "debug", skip(ws, src))]
pub fn completion(ws: &Workspace, from_fqn: &str, src: &str, offset: u32) -> Vec<CompletionItem> {
    let tokens = tokenize(src);

    // Inside a string literal or a doc/inner-doc comment the caret is in prose,
    // not code — offer nothing. (Without this, the token *before* the literal
    // governs, so typing in `given "…"` or a `///` line dumps the keyword set.)
    if inside_prose(&tokens, offset) {
        return Vec::new();
    }

    let trigger = governing_trigger(&tokens, offset);

    match trigger.map(|i| (i, tokens[i].kind)) {
        Some((i, TokenKind::Dot)) => member_items(ws, from_fqn, src, &tokens, i),
        Some((i, TokenKind::ColonColon)) => path_items(ws, from_fqn, &tokens, i),
        Some((_, TokenKind::HashLBracket)) => macro_items(),
        // A built-in macro's argument is a type path (`#[onevent(Event)]`).
        Some((i, TokenKind::LParen)) if is_macro_arg(&tokens, i) => type_items(ws),
        Some((_, TokenKind::Colon | TokenKind::LAngle)) => type_items(ws),
        // A `for` parent (or feature target) names a node, possibly cross-module.
        // A `container`'s parent must be a `system`, a `component`'s a `container`
        // (§4); only those kinds are offered. A feature target is any node.
        Some((i, TokenKind::KwFor)) => node_items(ws, from_fqn, parent_kind(&tokens, i)),
        _ => general_items(ws, from_fqn, offset),
    }
}

/// Visible node declarations (system / container / component / person) plus the
/// other modules — for a `for` parent or feature target.
fn node_items(ws: &Workspace, from_fqn: &str, required: Option<SymbolKind>) -> Vec<CompletionItem> {
    let nodes = ws
        .symbols()
        .filter(|s| {
            // A `for` with a known construct admits exactly one parent kind; a
            // feature target (no required kind) admits any node, never a `data`.
            let kind_ok = match required {
                Some(kind) => s.kind == kind,
                None => !matches!(s.kind, SymbolKind::Data),
            };
            kind_ok && (module_of(&s.fqn) == from_fqn || s.is_public)
        })
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn));
    nodes.chain(other_modules(ws, from_fqn)).collect()
}

/// The parent kind a `for` at `for_idx` admits, from the construct keyword two
/// tokens back (`container <name> for` → `system`; `component <name> for` →
/// `container`). `None` for a feature target or an incomplete construct — any node.
fn parent_kind(tokens: &[Token], for_idx: usize) -> Option<SymbolKind> {
    let name_idx = for_idx.checked_sub(1)?;
    if tokens[name_idx].kind != TokenKind::Ident {
        return None;
    }
    match tokens.get(name_idx.checked_sub(1)?)?.kind {
        TokenKind::KwContainer => Some(SymbolKind::System),
        TokenKind::KwComponent => Some(SymbolKind::Container),
        _ => None,
    }
}

/// The other workspace modules — local and dependency (§8.3) — offered as
/// cross-module reference starters (pick a module, then `::` drills into its
/// public symbols). Excludes the module being edited.
fn other_modules<'a>(
    ws: &'a Workspace,
    from_fqn: &'a str,
) -> impl Iterator<Item = CompletionItem> + 'a {
    ws.modules()
        .iter()
        .map(|m| m.fqn.as_str())
        .chain(ws.external_module_fqns())
        .filter(move |fqn| *fqn != from_fqn)
        .map(|fqn| item(fqn, CompletionKind::Module, "module"))
}

/// Whether `tokens[lparen]` opens a built-in macro's argument list (`#[name(`).
fn is_macro_arg(tokens: &[Token], lparen: usize) -> bool {
    lparen >= 2
        && tokens[lparen - 1].kind == TokenKind::Ident
        && tokens[lparen - 2].kind == TokenKind::HashLBracket
}

/// Whether `offset` falls strictly inside a string literal or a doc/inner-doc
/// comment — prose, where completion stays silent. Strict bounds (`start <
/// offset < end`) so the caret at a literal's edge still completes normally.
fn inside_prose(tokens: &[Token], offset: u32) -> bool {
    tokens.iter().any(|t| {
        matches!(
            t.kind,
            TokenKind::String | TokenKind::Doc | TokenKind::InnerDoc
        ) && t.span.start < offset
            && offset < t.span.end
    })
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
    src: &str,
    tokens: &[Token],
    dot: usize,
) -> Vec<CompletionItem> {
    // The token-based path covers `Module::Node.` and a bare binding.
    // A multi-step chain (`Repo.fetch(id).value.`) has no single base token, so
    // fall back to typing the receiver expression at the cursor via the AST.
    let owner = owner_before(ws, from_fqn, tokens, dot)
        .or_else(|| crate::infer::owner_at_dot(ws, from_fqn, src, tokens[dot].span.start));
    let Some((owner_module, owner_name)) = owner else {
        return Vec::new();
    };
    let Some(model) = ws.module_model(&owner_module) else {
        return Vec::new();
    };
    // A private member is reachable only within its own module (§8.2); across
    // modules only `public` members are offered.
    let same_module = owner_module == from_fqn;
    model
        .members(&owner_name)
        .iter()
        .filter(|m| same_module || m.is_public)
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
/// a `::`-qualified node in another module (`identity::sessions.`), or an
/// in-scope node name in this module.
fn owner_before(
    ws: &Workspace,
    from_fqn: &str,
    tokens: &[Token],
    dot: usize,
) -> Option<(String, String)> {
    let base = tokens.get(dot.checked_sub(1)?)?;
    match base.kind {
        // `a::b::Node.` — the base is qualified, so it names a symbol in the
        // module its qualifiers spell, not this one. Offered only when public
        // (or in this module), mirroring `::` path resolution (§8.2).
        TokenKind::Ident if dot >= 2 && tokens[dot - 2].kind == TokenKind::ColonColon => {
            let module = module_prefix(tokens, dot - 2);
            let symbol = ws.module_model(&module)?.symbol(&base.text)?;
            (module == from_fqn || symbol.is_public).then(|| (module, symbol.name.clone()))
        }
        TokenKind::Ident => {
            let module = ws.module(from_fqn)?;
            if let Some(symbol) = module.model.symbol(&base.text) {
                return Some((module_of(&symbol.fqn).to_owned(), symbol.name.clone()));
            }
            // Not a declared node — a local binding (`a = Repo.fetch(id); a.`):
            // resolve its inferred type to the node whose members it offers.
            let local = crate::infer::binding_type_at(ws, from_fqn, &base.text)?;
            crate::infer::type_owner(ws, from_fqn, &local.ty)
        }
        _ => None,
    }
}

/// Completions after a `::` whose left side is the module path `prefix`:
/// - the symbols of the module named exactly `prefix` (public only across
///   modules, §8.2), and
/// - the next path segment of any deeper module the prefix starts — so
///   `pseudoscript::` offers `cli`, `context`, … when the dependency's modules
///   are `pseudoscript::cli`, `pseudoscript::context`, … (a dependency root is
///   a path prefix, never a module itself).
fn path_items(
    ws: &Workspace,
    from_fqn: &str,
    tokens: &[Token],
    ccolon: usize,
) -> Vec<CompletionItem> {
    let prefix = module_prefix(tokens, ccolon);
    let mut items: Vec<CompletionItem> = ws
        .symbols()
        .filter(|s| module_of(&s.fqn) == prefix && (prefix == from_fqn || s.is_public))
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn))
        .collect();
    let with = format!("{prefix}::");
    let mut next: BTreeSet<&str> = BTreeSet::new();
    for fqn in ws
        .modules()
        .iter()
        .map(|m| m.fqn.as_str())
        .chain(ws.external_module_fqns())
    {
        if let Some(rest) = fqn.strip_prefix(&with) {
            next.insert(rest.split("::").next().unwrap_or(rest));
        }
    }
    items.extend(
        next.into_iter()
            .map(|seg| item(seg, CompletionKind::Module, "module")),
    );
    items
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
    let generics = ["Result", "Option"]
        .into_iter()
        .map(|g| item(g, CompletionKind::Type, "built-in generic"));
    let primitives = TokenKind::PRIMITIVE_TYPES
        .iter()
        .map(|t| item(t, CompletionKind::Type, "primitive type"))
        .chain(generics);
    let declared = ws
        .symbols()
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn));
    primitives.chain(declared).collect()
}

/// Keywords, this module's own symbols, the enclosing node's callables (bare
/// same-node calls, §5.1, ADR-041), and the other modules in the workspace (so a
/// cross-module reference can be started — pick the module, then `::` drills into
/// its public symbols).
fn general_items(ws: &Workspace, from_fqn: &str, offset: u32) -> Vec<CompletionItem> {
    let keywords = TokenKind::KEYWORDS
        .iter()
        .map(|k| item(k, CompletionKind::Keyword, "keyword"));
    let modules = other_modules(ws, from_fqn);
    let Some(entry) = ws.module(from_fqn) else {
        return keywords.chain(modules).collect();
    };
    let symbols = entry
        .model
        .symbols()
        .map(|s| item(&s.name, symbol_kind(s.kind), &s.fqn));
    // Inside a node body, the enclosing node's own callables are reachable as
    // bare same-node calls `Name(args)` — private siblings included (same node).
    let own_calls: Vec<CompletionItem> = enclosing_node(&entry.ast, offset)
        .map(|node| {
            entry
                .model
                .members(&node)
                .iter()
                .filter(|m| m.kind == MemberKind::Callable)
                .map(|m| item(&m.name, CompletionKind::Method, &m.detail))
                .collect()
        })
        .unwrap_or_default();
    keywords
        .chain(symbols)
        .chain(own_calls)
        .chain(modules)
        .collect()
}

/// The completion kind for a declared symbol.
fn symbol_kind(kind: SymbolKind) -> CompletionKind {
    match kind {
        SymbolKind::Data => CompletionKind::Class,
        SymbolKind::Constant => CompletionKind::Constant,
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

    fn workspace_with_externals(local: &[(&str, &str)], external: &[(&str, &str)]) -> Workspace {
        Workspace::build_with_externals(
            local
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
            external
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
        )
    }

    /// A dependency module `pseudoscript::cli` with a public node `LspHost` that
    /// has a public `run()` — the shape behind `pseudoscript::cli::LspHost.run()`.
    const DEP_CLI: (&str, &str) = (
        "pseudoscript::cli",
        "//! cli\npublic system Cli;\npublic component LspHost for pseudoscript::cli::Cli {\n  public run(): void;\n}\n",
    );

    #[test]
    fn dep_root_offers_its_submodule_segments() {
        // `pseudoscript::` — a dependency root is a path prefix, never a module
        // itself; completion offers the next segment (`cli`), not nothing.
        let src = "//! m\npublic system S;\npublic container X for pseudoscript::";
        let ws = workspace_with_externals(&[("m", src)], &[DEP_CLI]);
        let labels = labels_at(&ws, "m", src, src.len() as u32);
        assert!(labels.contains(&"cli".to_owned()), "{labels:?}");
    }

    #[test]
    fn dep_module_offers_its_public_symbols() {
        let src = "//! m\npublic system S;\npublic container X for pseudoscript::cli::";
        let ws = workspace_with_externals(&[("m", src)], &[DEP_CLI]);
        let labels = labels_at(&ws, "m", src, src.len() as u32);
        assert!(labels.contains(&"LspHost".to_owned()), "{labels:?}");
    }

    #[test]
    fn dep_node_offers_its_members_after_dot() {
        // `pseudoscript::cli::LspHost.` — members of a dependency node complete
        // (regression: the external module's model was dropped, so `.run` was
        // never offered).
        let src = "//! m\npublic container X for pseudoscript::cli::Cli {\n  go(): void {\n    pseudoscript::cli::LspHost.\n  }\n}\n";
        let ws = workspace_with_externals(&[("m", src)], &[DEP_CLI]);
        let offset = (src.find("LspHost.").unwrap() + "LspHost.".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"run".to_owned()), "{labels:?}");
    }

    /// Completion labels at byte `offset` in module `from`.
    fn labels_at(ws: &Workspace, from: &str, src: &str, offset: u32) -> Vec<String> {
        completion(ws, from, src, offset)
            .into_iter()
            .map(|c| c.label)
            .collect()
    }

    #[test]
    fn members_after_qualified_node_path() {
        // `identity::sessions.req` — a `::`-qualified node in another module;
        // its members must complete (regression: only the bare name was tried
        // in the current module).
        let mods = [
            (
                "identity",
                "//! identity\n\npublic system sessions {\n  public requireOrganizer(): void;\n}\n",
            ),
            (
                "m",
                "//! m\n\nsystem S {\n  run(): void {\n    identity::sessions.req\n  }\n}\n",
            ),
        ];
        let ws = workspace(&mods);
        let src = mods[1].1;
        let offset = (src.find("sessions.req").unwrap() + "sessions.req".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        // A `public` member completes across modules (§8.2).
        assert!(
            labels.contains(&"requireOrganizer".to_owned()),
            "{labels:?}"
        );
        // not the general keyword set
        assert!(
            !labels.contains(&"system".to_owned()),
            "general scope leaked: {labels:?}"
        );
    }

    #[test]
    fn members_after_local_binding() {
        // `a: Account = Repo.fetch(x); a.` — the binding's declared type drives
        // member completion, even though `a` is not a declared node.
        let src = "//! m\n\ndata Account { id: uuid, owner: string }\n\nsystem Repo {\n  fetch(x: uuid): Account;\n}\n\nsystem S {\n  run(): void {\n    a: Account = Repo.fetch(x)\n    a.\n  }\n}\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("a.\n").unwrap() + 2) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"id".to_owned()), "{labels:?}");
        assert!(labels.contains(&"owner".to_owned()), "{labels:?}");
    }

    #[test]
    fn option_offered_in_type_position() {
        let src = "//! m\n\ndata D { x: }\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("x:").unwrap() + 2) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"Option".to_owned()), "{labels:?}");
        assert!(labels.contains(&"Result".to_owned()), "{labels:?}");
    }

    #[test]
    fn siblings_complete_as_bare_calls() {
        // §5.1, ADR-041: inside a node body a sibling callable completes as a
        // bare same-node call `Name(args)` — no `self.` qualifier.
        let src =
            "//! m\n\nsystem S {\n  run(): void {\n    he\n  }\n  helper(x: number): uuid;\n}\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("    he").unwrap() + "    he".len()) as u32;
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
    fn general_offers_other_modules() {
        // At the root / a reference position, the other workspace modules are
        // suggested (so a cross-module reference can be started), alongside
        // keywords — but not this module itself.
        let mods = [
            ("context", "//! context\n\npublic system AcmeTickets;\n"),
            ("m", "//! m\n\n"),
        ];
        let ws = workspace(&mods);
        let src = mods[1].1;
        let labels = labels_at(&ws, "m", src, src.len() as u32);
        assert!(labels.contains(&"context".to_owned()), "{labels:?}");
        assert!(labels.contains(&"system".to_owned()), "{labels:?}");
        assert!(
            !labels.contains(&"m".to_owned()),
            "own module excluded: {labels:?}"
        );
    }

    #[test]
    fn dependency_modules_offered_as_cross_module_starters() {
        // A dependency module (§8.3) is offered in general position like any
        // other module, and `::` drills into its public symbols only.
        let ws = Workspace::build_with_externals(
            [("m", "//! m\n\n")]
                .iter()
                .map(|(f, s)| ((*f).to_owned(), parse(s).ast)),
            [(
                "banking::core",
                "//! banking::core\n\npublic system Ledger;\nsystem Hidden;\n",
            )]
            .iter()
            .map(|(f, s)| ((*f).to_owned(), parse(s).ast)),
        );

        let src = "//! m\n\n";
        let labels = labels_at(&ws, "m", src, src.len() as u32);
        assert!(labels.contains(&"banking::core".to_owned()), "{labels:?}");

        let drill = "//! m\n\ncontainer C for banking::core::\n";
        let offset = (drill.find("core::").unwrap() + "core::".len()) as u32;
        let drilled = labels_at(&ws, "m", drill, offset);
        assert!(drilled.contains(&"Ledger".to_owned()), "{drilled:?}");
        assert!(!drilled.contains(&"Hidden".to_owned()), "{drilled:?}");
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
