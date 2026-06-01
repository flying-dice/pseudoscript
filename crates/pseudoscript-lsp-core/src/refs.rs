//! Find-references, document-highlight, and rename.
//!
//! All three share one engine: resolve the definition under the cursor, then
//! scan every name-position identifier in the workspace and keep those that
//! resolve to the same definition. A `::` qualifier (an identifier followed by
//! `::`) names a module, not the symbol, so it is never an occurrence.

use pseudoscript_model::Workspace;
use pseudoscript_syntax::{Span, TokenKind, tokenize};

use crate::resolve::resolve_at;

/// One occurrence of a symbol: its span in module `fqn`'s source.
#[derive(Debug, Clone)]
pub struct Occurrence {
    /// FQN of the module the occurrence lies in.
    pub fqn: String,
    /// Span of the identifier in that module's source.
    pub span: Span,
}

/// Every occurrence of the symbol under `offset` across `modules`
/// (`(fqn, source)` pairs). With `include_decl` false, the defining occurrence
/// is omitted (find-references convention).
#[must_use]
pub fn references(
    ws: &Workspace,
    modules: &[(String, String)],
    from_fqn: &str,
    src: &str,
    offset: u32,
    include_decl: bool,
) -> Vec<Occurrence> {
    let Some(target) = resolve_at(ws, from_fqn, src, offset) else {
        return Vec::new();
    };
    let (target_mod, target_span) = (target.target_module, target.target_span);

    let mut out = Vec::new();
    for (fqn, source) in modules {
        let tokens = tokenize(source);
        for (i, token) in tokens.iter().enumerate() {
            if token.kind != TokenKind::Ident {
                continue;
            }
            // Skip module qualifiers (`a` in `a::Svc`); they name a module.
            if tokens
                .get(i + 1)
                .is_some_and(|t| t.kind == TokenKind::ColonColon)
            {
                continue;
            }
            let Some(hit) = resolve_at(ws, fqn, source, token.span.start) else {
                continue;
            };
            if !same(
                &hit.target_module,
                hit.target_span,
                &target_mod,
                target_span,
            ) {
                continue;
            }
            let is_decl = same(fqn, token.span, &target_mod, target_span);
            if include_decl || !is_decl {
                out.push(Occurrence {
                    fqn: fqn.clone(),
                    span: token.span,
                });
            }
        }
    }
    out
}

/// The occurrences of the symbol under `offset` within the current file only
/// (document highlight).
#[must_use]
pub fn highlights(ws: &Workspace, from_fqn: &str, src: &str, offset: u32) -> Vec<Span> {
    let modules = [(from_fqn.to_owned(), src.to_owned())];
    references(ws, &modules, from_fqn, src, offset, true)
        .into_iter()
        .filter(|occ| occ.fqn == from_fqn)
        .map(|occ| occ.span)
        .collect()
}

/// Whether two `(module, span)` pairs denote the same definition site.
fn same(a_mod: &str, a: Span, b_mod: &str, b: Span) -> bool {
    a_mod == b_mod && a.start == b.start && a.end == b.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_syntax::parse;

    fn ws(modules: &[(&str, &str)]) -> Workspace {
        Workspace::build(modules.iter().map(|(f, s)| ((*f).to_owned(), parse(s).ast)))
    }

    fn owned(modules: &[(&str, &str)]) -> Vec<(String, String)> {
        modules
            .iter()
            .map(|(f, s)| ((*f).to_owned(), (*s).to_owned()))
            .collect()
    }

    #[test]
    fn references_span_files_and_can_exclude_declaration() {
        let mods = [
            ("a", "//! a\n\npublic system Svc;\n"),
            ("b", "//! b\n\npublic container C for a::Svc;\n"),
        ];
        let workspace = ws(&mods);
        let owned = owned(&mods);
        // cursor on the declaration of Svc in module a
        let offset = mods[0].1.find("Svc").unwrap() as u32;

        let with_decl = references(&workspace, &owned, "a", mods[0].1, offset, true);
        assert_eq!(with_decl.len(), 2, "decl + cross-file use: {with_decl:?}");

        let without = references(&workspace, &owned, "a", mods[0].1, offset, false);
        assert_eq!(without.len(), 1, "use only: {without:?}");
        assert_eq!(without[0].fqn, "b");
    }

    #[test]
    fn references_to_an_operation_include_decl_and_calls() {
        // An operation's declaration and its `self.`-call are the same symbol.
        let src = "//! m\n\nsystem S {\n  run() { self.op() }\n  op() {}\n}\n";
        let mods = [("m", src)];
        let workspace = ws(&mods);
        let owned = owned(&mods);
        // cursor on the `op` declaration (the last `op(` in source)
        let offset = src.rfind("op(").unwrap() as u32;

        let with_decl = references(&workspace, &owned, "m", src, offset, true);
        assert_eq!(with_decl.len(), 2, "decl + self-call: {with_decl:?}");

        let without = references(&workspace, &owned, "m", src, offset, false);
        assert_eq!(without.len(), 1, "the call only: {without:?}");
    }

    #[test]
    fn cross_module_operation_references_span_files() {
        let mods = [
            ("a", "//! a\n\npublic system Svc {\n  op() {}\n}\n"),
            ("b", "//! b\n\nsystem App {\n  run() { a::Svc.op() }\n}\n"),
        ];
        let workspace = ws(&mods);
        let owned = owned(&mods);
        // cursor on the `op` declaration in module a
        let offset = mods[0].1.find("op(").unwrap() as u32;
        let refs = references(&workspace, &owned, "a", mods[0].1, offset, true);
        assert_eq!(refs.len(), 2, "decl in a + call in b: {refs:?}");
        assert!(
            refs.iter().any(|r| r.fqn == "b"),
            "call site in b: {refs:?}"
        );
    }

    #[test]
    fn highlights_are_confined_to_the_active_file() {
        let mods = [
            ("a", "//! a\n\npublic system Svc;\n"),
            ("b", "//! b\n\npublic container C for a::Svc;\n"),
        ];
        let workspace = ws(&mods);
        let offset = mods[1].1.find("Svc").unwrap() as u32 + 1;
        let spans = highlights(&workspace, "b", mods[1].1, offset);
        assert_eq!(spans.len(), 1, "only the reference in b");
    }
}
