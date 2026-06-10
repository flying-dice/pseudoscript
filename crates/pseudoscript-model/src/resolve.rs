//! Cursor resolution shared by hover and go-to-definition.
//!
//! Works at the token level so it survives partial parses, then consults the
//! resolved [`Workspace`] for the answer. It understands two reference forms:
//!
//! - a `::` path (bare name or full FQN) → the node/data it names (§8.2);
//! - a `.` member after `self` or a node name → that node's callable/field.
//!
//! A caret resting just past an identifier's last character still resolves it.

use crate::{Member, Symbol, Workspace, ast};
use pseudoscript_syntax::{Span, Token, TokenKind, tokenize};

/// A resolved cursor hit: where its definition lives and how to describe it.
pub struct Hit {
    /// Span of the clicked identifier in the active source (the hover range).
    pub clicked: Span,
    /// FQN of the module the definition lives in (maps to a file).
    pub target_module: String,
    /// Span of the definition within `target_module`'s source.
    pub target_span: Span,
    /// The resolved symbol's graph FQN — a node/data FQN, or for a member the
    /// owning node's FQN with `::member` appended (`banking::Ledger::Fetch`).
    /// The key a diagram projection (`pseudoscript_emit::project_symbol`) uses.
    pub target_fqn: String,
    /// A Markdown title line, e.g. ``system `banking::Bank` `` or ``run(id): uuid``.
    pub title: String,
    /// Doc summary or signature detail, shown under the title on hover.
    pub body: Option<String>,
}

/// Resolves the identifier under `offset` in module `from_fqn`'s `src`.
#[must_use]
#[tracing::instrument(level = "debug", skip(ws, src))]
pub fn resolve_at(ws: &Workspace, from_fqn: &str, src: &str, offset: u32) -> Option<Hit> {
    let tokens = tokenize(src);
    let idx = ident_index(&tokens, offset)?;
    let clicked = tokens[idx].span;

    // `.member` — resolve against the base's owning node.
    if idx >= 1 && tokens[idx - 1].kind == TokenKind::Dot {
        return resolve_member(ws, from_fqn, &tokens, idx).map(|hit| Hit { clicked, ..hit });
    }

    // A member's own declaration name (a callable or field) resolves to itself;
    // members live in the member index, not the symbol table, so this is the
    // only bare-name form that reaches them (ADR-004: no bare member references).
    if let Some((owner, member)) = ws
        .module(from_fqn)
        .and_then(|entry| entry.model.member_at(clicked))
    {
        let owner_fqn = ws
            .module(from_fqn)
            .and_then(|entry| entry.model.symbol(owner))
            .map_or_else(|| owner.to_owned(), |s| s.fqn.clone());
        return Some(Hit {
            clicked,
            ..member_hit(member, owner, &owner_fqn, from_fqn.to_owned())
        });
    }

    // Otherwise the clicked token is part of a `::` path (possibly one segment).
    let segments = path_segments(&tokens, idx);
    resolve_path(ws, from_fqn, &segments).map(|hit| Hit { clicked, ..hit })
}

/// The index of the identifier token at `offset`, or the one ending exactly at
/// `offset` (caret just past the word).
fn ident_index(tokens: &[Token], offset: u32) -> Option<usize> {
    let is_ident = |t: &Token| t.kind == TokenKind::Ident;
    tokens
        .iter()
        .position(|t| is_ident(t) && t.span.start <= offset && offset < t.span.end)
        .or_else(|| {
            offset.checked_sub(1).and_then(|prev| {
                tokens
                    .iter()
                    .position(|t| is_ident(t) && t.span.start <= prev && prev < t.span.end)
            })
        })
}

/// The contiguous `Ident (:: Ident)*` path the token at `idx` belongs to, as
/// segment texts in source order.
fn path_segments(tokens: &[Token], idx: usize) -> Vec<&str> {
    let mut start = idx;
    while start >= 2
        && tokens[start - 1].kind == TokenKind::ColonColon
        && tokens[start - 2].kind == TokenKind::Ident
    {
        start -= 2;
    }
    let mut end = idx;
    while end + 2 < tokens.len()
        && tokens[end + 1].kind == TokenKind::ColonColon
        && tokens[end + 2].kind == TokenKind::Ident
    {
        end += 2;
    }
    (start..=end)
        .step_by(2)
        .map(|i| tokens[i].text.as_str())
        .collect()
}

/// Resolves a `::` path to its declared node/data symbol.
fn resolve_path(ws: &Workspace, from_fqn: &str, segments: &[&str]) -> Option<Hit> {
    let symbol = resolve_node(ws, from_fqn, segments)?;
    Some(symbol_hit(ws, symbol, module_of(&symbol.fqn)))
}

/// Resolves a `::` path to the node/data [`Symbol`] it names: the local symbol
/// for a bare name, the global FQN index for a qualified one.
fn resolve_node<'a>(ws: &'a Workspace, from_fqn: &str, segments: &[&str]) -> Option<&'a Symbol> {
    if let [name] = segments {
        let model = &ws.module(from_fqn)?.model;
        if let Some(symbol) = model.symbol(name) {
            return Some(symbol);
        }
        // Best-effort: a bare name declared in another module — an
        // under-qualified cross-module reference (§8 wants an FQN) —
        // navigates to its declaration when exactly one *visible* workspace
        // symbol bears that name. Goto leniency only; the static checker still
        // flags the missing qualifier, and an ambiguous name is left unresolved.
        return unique_symbol(ws, from_fqn, name);
    }
    // A multi-segment path must be the flat FQN `module::Name` (§8.1, ADR-030):
    // it resolves only as an exact, visible symbol. A wrong or structural-drill
    // qualifier (`Syntax::Lexer` for module `syntax`) names no symbol and
    // resolves nowhere — the checker reports it; goto does not paper over it by
    // guessing the leaf.
    let symbol = ws.symbol(&segments.join("::"))?;
    // §8: a private symbol is reachable only within its own module, even by FQN.
    visible_from(symbol, from_fqn).then_some(symbol)
}

/// Whether `symbol` is reachable from module `from_fqn` (§8.2): a same-module
/// declaration is always visible within its own file; a cross-module reference
/// resolves only to a `public` symbol.
fn visible_from(symbol: &Symbol, from_fqn: &str) -> bool {
    module_of(&symbol.fqn) == from_fqn || symbol.is_public
}

/// The `(module, name)` of the node/data a `::` path names, for type inference
/// (the owner whose members a `.` chain walks).
#[must_use]
pub fn resolve_owner(
    ws: &Workspace,
    from_fqn: &str,
    segments: &[&str],
) -> Option<(String, String)> {
    let symbol = resolve_node(ws, from_fqn, segments)?;
    Some((module_of(&symbol.fqn).to_owned(), symbol.name.clone()))
}

/// The single *local* workspace symbol named `name`, or `None` if there are zero
/// or more than one (an ambiguous bare reference is not guessed).
///
/// Dependency (external) symbols are excluded: a cross-workspace reference MUST
/// be qualified `dep::module::Node` (§8.3), so a bare name never leniently
/// resolves into a dependency — that would over-resolve common identifiers into
/// a dependency's symbols (wrong goto, every same-named token highlighted).
fn unique_symbol<'a>(ws: &'a Workspace, from_fqn: &str, name: &str) -> Option<&'a Symbol> {
    let mut matches = ws.symbols().filter(|s| {
        s.name == name && !ws.is_external_module(module_of(&s.fqn)) && visible_from(s, from_fqn)
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// Resolves `.member` after a `self` or node/data base — including a qualified,
/// cross-module base (`a::Svc.op`).
fn resolve_member(ws: &Workspace, from_fqn: &str, tokens: &[Token], idx: usize) -> Option<Hit> {
    let member_name = tokens[idx].text.as_str();
    // `idx - 1` is the `.`; the base ends at `idx - 2`.
    let base_idx = idx.checked_sub(2)?;
    let base = tokens.get(base_idx)?;

    let symbol = match base.kind {
        TokenKind::KwSelf => {
            let node = enclosing_node(&ws.module(from_fqn)?.ast, base.span.start)?;
            ws.module(from_fqn)?.model.symbol(&node)?
        }
        TokenKind::Ident => {
            let segments = path_segments(tokens, base_idx);
            resolve_node(ws, from_fqn, &segments)?
        }
        _ => return None,
    };

    let owner_module = module_of(&symbol.fqn);
    let member = ws
        .module_model(owner_module)?
        .members(&symbol.name)
        .iter()
        .find(|m| m.name == member_name)?;
    Some(member_hit(
        member,
        &symbol.name,
        &symbol.fqn,
        owner_module.to_owned(),
    ))
}

/// Builds a [`Hit`] for a declared node/data symbol, with its doc summary.
fn symbol_hit(ws: &Workspace, symbol: &Symbol, target_module: &str) -> Hit {
    let body = ws
        .module_any(target_module)
        .and_then(|entry| doc_summary(&entry.ast, &symbol.name));
    Hit {
        clicked: Span::new(0, 0),
        target_module: target_module.to_owned(),
        target_span: symbol.span,
        target_fqn: symbol.fqn.clone(),
        title: format!("{} `{}`", symbol.kind.keyword(), symbol.fqn),
        body,
    }
}

/// Builds a [`Hit`] for a node member (callable or field): its signature, then
/// the callable's `///` summary when present. `owner_fqn` is the owning node's
/// FQN; the member's graph FQN is that with `::member` appended.
fn member_hit(member: &Member, owner: &str, owner_fqn: &str, owner_module: String) -> Hit {
    let kind = match member.kind {
        crate::MemberKind::Callable => "callable",
        crate::MemberKind::Field => "field",
    };
    let mut body = member.detail.clone();
    if let Some(doc) = &member.doc {
        body.push_str("\n\n");
        body.push_str(doc);
    }
    Hit {
        clicked: Span::new(0, 0),
        target_module: owner_module,
        target_span: member.span,
        target_fqn: format!("{owner_fqn}::{}", member.name),
        title: format!("{kind} `{owner}.{}`", member.name),
        body: Some(body),
    }
}

/// The module portion of an FQN (`a::b::C` → `a::b`); empty for a bare name.
#[must_use]
pub fn module_of(fqn: &str) -> &str {
    fqn.rsplit_once("::").map_or("", |(module, _)| module)
}

/// The name of the innermost node whose span contains `offset` (the enclosing
/// node for a `self` reference).
#[must_use]
pub fn enclosing_node(module: &ast::Module, offset: u32) -> Option<String> {
    fn visit(decl: &ast::Decl, offset: u32, found: &mut Option<String>) {
        let (ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node)) = &decl.kind
        else {
            return;
        };
        if node.span.start <= offset && offset < node.span.end {
            *found = Some(node.name.name.clone());
            for member in node.body.iter().flatten() {
                if let ast::BodyMember::Decl(inner) = member {
                    visit(inner, offset, found);
                }
            }
        }
    }

    let mut found = None;
    for item in &module.items {
        if let ast::Item::Decl(decl) = item {
            visit(decl, offset, &mut found);
        }
    }
    found
}

/// The `///` summary of the declaration named `name` in `module`, if any.
fn doc_summary(module: &ast::Module, name: &str) -> Option<String> {
    fn from_decl(decl: &ast::Decl, name: &str) -> Option<String> {
        let decl_name = match &decl.kind {
            ast::DeclKind::Person(n)
            | ast::DeclKind::System(n)
            | ast::DeclKind::Container(n)
            | ast::DeclKind::Component(n) => &n.name.name,
            ast::DeclKind::Data(d) => &d.name.name,
            ast::DeclKind::Constant(c) => &c.name.name,
        };
        if decl_name == name && !decl.doc.summary.is_empty() {
            return Some(decl.doc.summary.join(" "));
        }
        if let ast::DeclKind::Person(n)
        | ast::DeclKind::System(n)
        | ast::DeclKind::Container(n)
        | ast::DeclKind::Component(n) = &decl.kind
        {
            for member in n.body.iter().flatten() {
                if let ast::BodyMember::Decl(inner) = member
                    && let Some(found) = from_decl(inner, name)
                {
                    return Some(found);
                }
            }
        }
        None
    }

    module.items.iter().find_map(|item| match item {
        ast::Item::Decl(decl) => from_decl(decl, name),
        ast::Item::Feature(_) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_syntax::parse;

    /// Builds a workspace from `(fqn, source)` modules.
    fn workspace(modules: &[(&str, &str)]) -> Workspace {
        Workspace::build(
            modules
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
        )
    }

    /// Resolves the cursor at the first occurrence of `needle` (plus `at` bytes
    /// into it) in module `from`.
    fn hit_at(ws: &Workspace, from: &str, src: &str, needle: &str, at: u32) -> Hit {
        let offset = src.find(needle).expect("needle present") as u32 + at;
        resolve_at(ws, from, src, offset).unwrap_or_else(|| panic!("no hit at {needle:?}"))
    }

    /// The source slice the target span covers, for asserting where goto lands.
    fn slice<'a>(modules: &[(&'a str, &'a str)], hit: &Hit) -> &'a str {
        let src = modules
            .iter()
            .find(|(fqn, _)| *fqn == hit.target_module)
            .expect("target module present")
            .1;
        &src[hit.target_span.start as usize..hit.target_span.end as usize]
    }

    #[test]
    fn cross_file_fqn_resolves_to_other_module() {
        let mods = [
            ("a", "//! a\n\npublic system Svc;\n"),
            ("b", "//! b\n\npublic container C for a::Svc;\n"),
        ];
        let ws = workspace(&mods);
        let hit = hit_at(&ws, "b", mods[1].1, "Svc", 1);
        assert_eq!(hit.target_module, "a");
        assert_eq!(slice(&mods, &hit), "Svc");
        assert!(hit.title.contains("system `a::Svc`"));
    }

    #[test]
    fn self_member_resolves_to_callable() {
        let src = "//! m\n\nsystem S {\n  run(): void { self.helper() }\n  helper(): void {}\n}\n";
        let mods = [("m", src)];
        let ws = workspace(&mods);
        let hit = hit_at(&ws, "m", src, "helper", 1);
        assert_eq!(slice(&mods, &hit), "helper");
        assert!(hit.title.contains("callable `S.helper`"));
    }

    #[test]
    fn node_qualified_member_resolves_to_callable() {
        let src = "//! m\n\nsystem Repo {\n  fetch(): void {}\n}\n\nsystem App {\n  run(): void { Repo.fetch() }\n}\n";
        let mods = [("m", src)];
        let ws = workspace(&mods);
        // click `fetch` in the `Repo.fetch()` call, not the declaration
        let call = src.find("Repo.fetch").unwrap() + "Repo.".len();
        let hit = resolve_at(&ws, "m", src, call as u32 + 1).expect("member resolves");
        assert_eq!(slice(&mods, &hit), "fetch");
        assert!(hit.title.contains("callable `Repo.fetch`"), "{}", hit.title);
    }

    #[test]
    fn qualified_component_member_call_resolves_cross_module() {
        // Mirrors `cli::DocCmd.run(path)` called from another module.
        let cli = "//! cli\n\npublic container Cli;\n\npublic component DocCmd for Cli {\n  public run(path: string): void {\n    self.write(path)\n  }\n  write(p: string): void;\n}\n";
        let ctx = "//! context\n\npublic person Developer {\n  public renderDocs(path: string): void {\n    cli::DocCmd.run(path)\n  }\n}\n";
        let mods = [("cli", cli), ("context", ctx)];
        let ws = workspace(&mods);
        let call = ctx.find("cli::DocCmd.run").unwrap() + "cli::DocCmd.".len();
        let hit = resolve_at(&ws, "context", ctx, call as u32 + 1).expect("member call resolves");
        assert_eq!(hit.target_module, "cli");
        assert_eq!(slice(&mods, &hit), "run");
        assert!(hit.title.contains("callable `DocCmd.run`"), "{}", hit.title);
    }

    #[test]
    fn cross_module_member_resolves() {
        let mods = [
            ("a", "//! a\n\npublic system Svc {\n  op(): void {}\n}\n"),
            (
                "b",
                "//! b\n\nsystem App {\n  run(): void { a::Svc.op() }\n}\n",
            ),
        ];
        let ws = workspace(&mods);
        let src = mods[1].1;
        let call = src.find("a::Svc.op").unwrap() + "a::Svc.".len();
        let hit = resolve_at(&ws, "b", src, call as u32 + 1).expect("cross-module member resolves");
        assert_eq!(hit.target_module, "a");
        assert_eq!(slice(&mods, &hit), "op");
    }

    #[test]
    fn private_node_is_not_reachable_cross_module() {
        // §8: a private node is reachable only within its own module, even by FQN.
        // A cross-module reference resolves nowhere (the checker flags it); a
        // public sibling resolves.
        let mods = [
            ("a", "//! a\n\nsystem Hidden;\npublic system Shown;\n"),
            (
                "b",
                "//! b\n\npublic container P for a::Hidden;\npublic container Q for a::Shown;\n",
            ),
        ];
        let ws = workspace(&mods);
        let b = mods[1].1;
        let hidden = b.find("a::Hidden").unwrap() + "a::".len();
        assert!(
            resolve_at(&ws, "b", b, hidden as u32 + 1).is_none(),
            "private node leaked across modules"
        );
        let shown = b.find("a::Shown").unwrap() + "a::".len();
        let hit = resolve_at(&ws, "b", b, shown as u32 + 1).expect("public node resolves");
        assert_eq!(hit.target_module, "a");
        assert_eq!(slice(&mods, &hit), "Shown");
    }

    #[test]
    fn private_node_resolves_within_its_own_module() {
        // Same-module: a private node IS reachable by its own FQN within its file.
        let mods = [(
            "a",
            "//! a\n\nsystem Hidden;\npublic container C for a::Hidden;\n",
        )];
        let ws = workspace(&mods);
        let a = mods[0].1;
        let h = a.find("for a::Hidden").unwrap() + "for a::".len();
        let hit = resolve_at(&ws, "a", a, h as u32 + 1).expect("same-module private resolves");
        assert_eq!(slice(&mods, &hit), "Hidden");
    }

    #[test]
    fn member_hover_includes_signature_and_docstring() {
        let src = "//! m\n\nsystem S {\n  /// The token stream a parser consumes.\n  tokenize(text: string): string;\n}\n";
        let mods = [("m", src)];
        let ws = workspace(&mods);
        let hit = hit_at(&ws, "m", src, "tokenize", 1);
        let body = hit.body.expect("member body");
        assert!(
            body.contains("tokenize(text: string): string"),
            "signature: {body}"
        );
        assert!(
            body.contains("The token stream a parser consumes."),
            "doc: {body}"
        );
    }

    #[test]
    fn callable_declaration_resolves_to_itself() {
        let src = "//! m\n\nsystem S {\n  op(): void {}\n}\n";
        let mods = [("m", src)];
        let ws = workspace(&mods);
        let hit = hit_at(&ws, "m", src, "op", 1);
        assert_eq!(slice(&mods, &hit), "op");
        assert!(hit.title.contains("callable `S.op`"), "{}", hit.title);
    }

    #[test]
    fn field_declaration_resolves_to_itself() {
        let src = "//! m\n\ndata Rec { id: uuid }\n";
        let mods = [("m", src)];
        let ws = workspace(&mods);
        let hit = hit_at(&ws, "m", src, "id", 1);
        assert_eq!(slice(&mods, &hit), "id");
        assert!(hit.title.contains("field `Rec.id`"), "{}", hit.title);
    }

    #[test]
    fn structural_drill_member_does_not_resolve_but_flat_does() {
        // `Syntax::Lexer.tokenize` is a structural drill (container `Syntax`,
        // component `Lexer`), not the flat FQN `syntax::Lexer` (§8.1, ADR-030):
        // it resolves nowhere. The flat form resolves.
        let drill = "//! syntax\n\npublic container Syntax;\npublic component Lexer for syntax::Syntax {\n  tokenize(t: string): string;\n}\n\npublic component Parser for syntax::Syntax {\n  go(): void { Syntax::Lexer.tokenize(\"x\") }\n}\n";
        let dmods = [("syntax", drill)];
        let dws = workspace(&dmods);
        let dcall = drill.find("Syntax::Lexer.tokenize").unwrap() + "Syntax::Lexer.".len();
        assert!(
            resolve_at(&dws, "syntax", drill, dcall as u32 + 1).is_none(),
            "a structural drill must not resolve"
        );

        let flat = "//! syntax\n\npublic container Syntax;\npublic component Lexer for syntax::Syntax {\n  tokenize(t: string): string;\n}\n\npublic component Parser for syntax::Syntax {\n  go(): void { syntax::Lexer.tokenize(\"x\") }\n}\n";
        let mods = [("syntax", flat)];
        let ws = workspace(&mods);
        let call = flat.find("syntax::Lexer.tokenize").unwrap() + "syntax::Lexer.".len();
        let hit =
            resolve_at(&ws, "syntax", flat, call as u32 + 1).expect("flat FQN member resolves");
        assert_eq!(slice(&mods, &hit), "tokenize");
        assert!(
            hit.title.contains("callable `Lexer.tokenize`"),
            "{}",
            hit.title
        );
    }

    #[test]
    fn bare_cross_module_type_resolves_when_unique() {
        // `ast: Module` where `Module` is declared in another module: an
        // under-qualified reference still navigates to the one matching shape.
        let mods = [
            ("syntax", "//! syntax\n\npublic data Module;\n"),
            (
                "parser",
                "//! parser\n\npublic data Parsed { ast: Module }\n",
            ),
        ];
        let ws = workspace(&mods);
        let offset = mods[1].1.find("ast: Module").unwrap() + "ast: ".len();
        let hit = resolve_at(&ws, "parser", mods[1].1, offset as u32 + 1).expect("type resolves");
        assert_eq!(hit.target_module, "syntax");
        assert_eq!(slice(&mods, &hit), "Module");
        assert!(hit.title.contains("data `syntax::Module`"), "{}", hit.title);
    }

    #[test]
    fn bare_name_does_not_leniently_resolve_into_a_dependency() {
        // A dependency exports `Money`; the consumer uses it *unqualified*. §8.3
        // requires `dep::core::Money`, so the bare name must NOT resolve into the
        // dependency (otherwise goto jumps into a dep and every same-named token
        // is highlighted). A qualified path still resolves.
        let ws = Workspace::build_with_externals(
            [("m", "//! m\n\npublic data Rec { x: Money }\n")]
                .iter()
                .map(|(f, s)| ((*f).to_owned(), parse(s).ast)),
            [(
                "dep::core",
                "//! dep::core\n\npublic data Money { amount: number }\n",
            )]
            .iter()
            .map(|(f, s)| ((*f).to_owned(), parse(s).ast)),
        );
        let src = "//! m\n\npublic data Rec { x: Money }\n";
        let bare = src.find(": Money").unwrap() as u32 + 2;
        assert!(
            resolve_at(&ws, "m", src, bare).is_none(),
            "bare name wrongly resolved into a dependency"
        );

        // The qualified reference resolves (via the FQN index, unaffected).
        let qsrc = "//! m\n\npublic data Rec { x: dep::core::Money }\n";
        let qoff = qsrc.find("core::Money").unwrap() as u32 + "core::".len() as u32 + 1;
        let hit = resolve_at(&ws, "m", qsrc, qoff).expect("qualified dep ref resolves");
        assert_eq!(hit.target_module, "dep::core");
    }

    #[test]
    fn qualified_dependency_node_and_member_resolve_with_doc() {
        // `pseudoscript::cli::LspHost.run()` from a consumer: the node hover
        // carries the dependency's `///` summary, and the member resolves to its
        // declaration — both reach the external module's retained AST + model.
        let ws = Workspace::build_with_externals(
            [("m", "//! m\n")]
                .iter()
                .map(|(f, s)| ((*f).to_owned(), parse(s).ast)),
            [(
                "pseudoscript::cli",
                "//! cli\npublic system Cli;\n/// The stdio language server host.\npublic component LspHost for pseudoscript::cli::Cli {\n  /// Start the server.\n  public run(): void;\n}\n",
            )]
            .iter()
            .map(|(f, s)| ((*f).to_owned(), parse(s).ast)),
        );

        // Hover the node: title + its doc summary from the dependency's source.
        let src = "//! m\npublic container X for pseudoscript::cli::LspHost;\n";
        let off = src.find("LspHost").unwrap() as u32 + 1;
        let hit = resolve_at(&ws, "m", src, off).expect("dep node resolves");
        assert_eq!(hit.target_module, "pseudoscript::cli");
        assert_eq!(hit.target_fqn, "pseudoscript::cli::LspHost");
        assert_eq!(hit.body.as_deref(), Some("The stdio language server host."));

        // Resolve the member `run` on the dependency node.
        let msrc = "//! m\npublic container X for pseudoscript::cli::Cli {\n  go(): void {\n    pseudoscript::cli::LspHost.run()\n  }\n}\n";
        let moff = msrc.find("LspHost.run").unwrap() as u32 + "LspHost.".len() as u32 + 1;
        let mhit = resolve_at(&ws, "m", msrc, moff).expect("dep member resolves");
        assert_eq!(mhit.target_fqn, "pseudoscript::cli::LspHost::run");
    }

    #[test]
    fn ambiguous_bare_type_is_not_guessed() {
        // Two `Module`s in different modules: a bare reference resolves to
        // neither (the FQN is required to disambiguate, §8).
        let mods = [
            ("a", "//! a\n\npublic data Module;\n"),
            ("b", "//! b\n\npublic data Module;\n"),
            ("c", "//! c\n\npublic data Parsed { ast: Module }\n"),
        ];
        let ws = workspace(&mods);
        let offset = mods[2].1.find("ast: Module").unwrap() + "ast: ".len();
        assert!(resolve_at(&ws, "c", mods[2].1, offset as u32 + 1).is_none());
    }

    #[test]
    fn caret_just_past_identifier_still_resolves() {
        let src = "//! m\n\npublic system Banking;\n";
        let mods = [("m", src)];
        let ws = workspace(&mods);
        // offset at the character just after "Banking"
        let end = (src.find("Banking").unwrap() + "Banking".len()) as u32;
        let hit = resolve_at(&ws, "m", src, end).expect("end caret resolves");
        assert_eq!(slice(&mods, &hit), "Banking");
    }
}
