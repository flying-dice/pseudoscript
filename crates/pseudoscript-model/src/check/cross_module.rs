//! Cross-module visibility resolution (`LANG.md` §8.2, ADR-010).
//!
//! The single-module checks ([`super`]) cannot see other files, so they skip
//! references they cannot resolve locally. This pass runs over a whole
//! [`Workspace`]: it walks each module's qualified references — `for` parents,
//! `alias`/`feature` targets, body call targets, and type annotations (field,
//! parameter, return, and generic-argument types) — and against the global FQN
//! index enforces §8.2: a reference from module A to a symbol in module B
//! resolves only if that symbol is `public`. A cross-module reference to a
//! private symbol, or a dangling cross-module FQN, is a diagnostic.
//!
//! References whose target lives in the *same* module are left to the
//! single-module checks; only genuinely cross-module FQNs are judged here, so
//! no single-module scenario gains or loses a diagnostic.

use pseudoscript_syntax::Diagnostic;
use pseudoscript_syntax::ast::{
    Block, BodyMember, Callable, DataBody, DeclKind, Expr, ExprKind, Item, Node, Path, PostfixSeg,
    Ref, Stmt, StmtKind, Type,
};

use crate::model::{ModuleEntry, Resolution, SymbolKind, Workspace};

/// Emits a diagnostic for every cross-module reference that resolves to a
/// private node or to nothing.
pub(crate) fn check(workspace: &Workspace) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for entry in workspace.modules() {
        check_module(workspace, entry, &mut out);
    }
    out
}

/// The cross-module diagnostics originating in a single module. Every span lies
/// in `entry`'s source, so the LSP can attribute them to that file's URI.
pub(crate) fn check_one(workspace: &Workspace, entry: &ModuleEntry) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check_module(workspace, entry, &mut out);
    out
}

fn check_module(workspace: &Workspace, entry: &ModuleEntry, out: &mut Vec<Diagnostic>) {
    let mut ctx = Ctx {
        workspace,
        from_module: &entry.fqn,
        out,
    };
    for item in &entry.ast.items {
        match item {
            Item::Decl(decl) => ctx.check_decl_kind(&decl.kind),
            // §5.2: a cross-module feature target must be `public`.
            Item::Feature(feature) => ctx.check_ref(&feature.target, "feature target"),
        }
    }
}

struct Ctx<'a> {
    workspace: &'a Workspace,
    from_module: &'a str,
    out: &'a mut Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn check_decl_kind(&mut self, kind: &DeclKind) {
        match kind {
            DeclKind::Container(node) | DeclKind::Component(node) => {
                if let Some(parent) = &node.parent {
                    self.check_ref(parent, "parent");
                }
                self.check_node_body(node);
            }
            DeclKind::Person(node) | DeclKind::System(node) => self.check_node_body(node),
            DeclKind::Data(data) => self.check_data_types(&data.body),
        }
    }

    fn check_node_body(&mut self, node: &Node) {
        let Some(members) = &node.body else { return };
        for member in members {
            match member {
                BodyMember::Callable(callable) => self.check_callable(callable),
                BodyMember::Decl(decl) => self.check_decl_kind(&decl.kind),
            }
        }
    }

    fn check_callable(&mut self, callable: &Callable) {
        for param in &callable.params {
            self.check_type(&param.ty);
        }
        if let Some(ret) = &callable.return_ty {
            self.check_type(ret);
        }
        if let Some(body) = &callable.body {
            self.check_block(body);
        }
    }

    /// §3.3/§8.2: a qualified type — a field, parameter, or return type, and each
    /// generic argument — resolves cross-module under the same visibility rule as
    /// a node reference. A bare type is the single-module checks' business.
    fn check_data_types(&mut self, body: &DataBody) {
        match body {
            DataBody::Record(fields) => {
                for field in fields {
                    self.check_type(&field.ty);
                }
            }
            DataBody::Union(variants) => {
                for variant in variants {
                    for field in variant.record.iter().flatten() {
                        self.check_type(&field.ty);
                    }
                }
            }
            DataBody::BlackBox => {}
        }
    }

    fn check_type(&mut self, ty: &Type) {
        self.check_ref(&ty.name, "type");
        for generic in &ty.generics {
            self.check_type(generic);
        }
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Assign { value, .. } | StmtKind::Expr(value) => self.check_expr(value),
            StmtKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.check_expr(expr);
                }
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.check_expr(cond);
                self.check_block(then_block);
                if let Some(else_block) = else_block {
                    self.check_block(else_block);
                }
            }
            StmtKind::For { iter, body, .. } => {
                self.check_expr(iter);
                self.check_block(body);
            }
            StmtKind::While { cond, body } => {
                self.check_expr(cond);
                self.check_block(body);
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Postfix { base, segments } => {
                // The call target is the base of the chain (`Model::Builder` in
                // `Model::Builder.build(...)`); only it names a node.
                if let ExprKind::Ref(Ref::Path(path)) = &base.kind {
                    self.check_ref(path, "call target");
                    self.check_fqn_member(path, segments);
                } else {
                    self.check_expr(base);
                }
                for seg in segments {
                    if let Some(args) = &seg.call_args {
                        for arg in args {
                            self.check_expr(arg);
                        }
                    }
                }
            }
            ExprKind::From { source, .. } => {
                for src in source.sources() {
                    self.check_expr(src);
                }
            }
            ExprKind::Marker { payload, .. } => {
                if let Some(payload) = payload {
                    self.check_expr(payload);
                }
            }
            ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) => self.check_expr(expr),
            ExprKind::Ref(Ref::Path(path)) => self.check_value_ref(path),
            ExprKind::Ref(Ref::SelfNode(_)) | ExprKind::Literal(_) => {}
        }
    }

    /// §3.5 / ADR-032: a value-position multi-segment path is a union-variant
    /// reference — the operand of `Ok`/`Err`/`Some`, a `from` source, a `return`
    /// value. It MUST name an existing variant:
    ///
    /// - a **record** variant hoists, so the whole path `module::Variant`
    ///   resolves as a `data` symbol;
    /// - a **fieldless** variant does not hoist, so `module::Union::Variant`
    ///   names the variant through its union — the leaf MUST be a fieldless
    ///   variant of the union the prefix names.
    ///
    /// A path that resolves to neither — an unknown union, a private one, or a
    /// leaf no variant of it — is rejected. A bare name is a local (parameter,
    /// binding, `for` binding) and the single-module checks' business.
    fn check_value_ref(&mut self, path: &Path) {
        if path.is_simple() {
            return;
        }
        let fqn = path_str(path);
        // A record variant (or any hoisted `data`) resolves directly — but as a
        // value it is not one: a `data` value is produced with `from`, and a node
        // is never a value (ADR-035).
        match self.workspace.resolve_qualified(self.from_module, &fqn) {
            Resolution::Public(symbol) => {
                let leaf = symbol.name.as_str();
                let message = match symbol.kind {
                    SymbolKind::Data => format!("`{leaf}` is not a value: compose it with `from`"),
                    _ => format!("`{leaf}` is a node, not a value"),
                };
                self.out.push(Diagnostic::error(path.span, message));
                return;
            }
            Resolution::Private(_) => {
                self.out.push(Diagnostic::error(
                    path.span,
                    format!("variant `{fqn}` is private to its module"),
                ));
                return;
            }
            Resolution::Missing => {}
        }
        // Not a hoisted symbol: the leaf must be a fieldless variant addressed
        // through its union `module::Union`.
        let Some((union_fqn, variant)) = fqn.rsplit_once("::") else {
            return;
        };
        match self
            .workspace
            .resolve_qualified(self.from_module, union_fqn)
        {
            Resolution::Public(_) => {
                let known = union_fqn
                    .rsplit_once("::")
                    .and_then(|(module, union)| {
                        self.workspace
                            .module_model(module)
                            .map(|m| m.has_fieldless_variant(union, variant))
                    })
                    .unwrap_or(false);
                if !known {
                    self.out.push(Diagnostic::error(
                        path.span,
                        format!("`{union_fqn}` has no fieldless variant `{variant}`"),
                    ));
                }
            }
            Resolution::Private(_) => self.out.push(Diagnostic::error(
                path.span,
                format!("variant union `{union_fqn}` is private to its module"),
            )),
            Resolution::Missing => self.out.push(Diagnostic::error(
                path.span,
                format!("variant reference `{fqn}` does not resolve: no union `{union_fqn}`"),
            )),
        }
    }

    /// §6 / ADR-022: the first segment of a postfix chain rooted at a node FQN
    /// MUST name a member of that node — a call segment a callable, a field
    /// segment a field. The single-module checks resolve members only on a bare
    /// (same-module) receiver; a `module::Node` base is theirs to skip and this
    /// pass's to judge. Member *visibility* gates completion, not callability
    /// (§8.2), so existence — not `public` — is the test. Deeper segments chain
    /// off a member type that is not inferred cross-module and are left.
    fn check_fqn_member(&mut self, base: &Path, segments: &[PostfixSeg]) {
        if base.is_simple() {
            return;
        }
        let fqn = path_str(base);
        let Resolution::Public(symbol) = self.workspace.resolve_qualified(self.from_module, &fqn)
        else {
            return; // a private/missing base is reported by `check_ref`
        };
        // A `data` FQN is a value form (resolved via its binding), not a node
        // receiver; only a node owns callable/field members.
        if symbol.kind == SymbolKind::Data {
            return;
        }
        let (Some(seg), Some((module, node))) = (segments.first(), fqn.rsplit_once("::")) else {
            return;
        };
        let Some(model) = self.workspace.module_model(module) else {
            return;
        };
        let members = model.members(node);
        // A black-box node (`;`, no disclosed body) exposes no members; its
        // membership is unknown, so absence cannot be proven (ADR-022). Only a
        // node with a disclosed member set can reject a name.
        if members.is_empty() || members.iter().any(|m| m.name == seg.name.name) {
            return;
        }
        let what = if seg.call_args.is_some() {
            "method"
        } else {
            "field"
        };
        self.out.push(Diagnostic::error(
            seg.span,
            format!("no {what} `{}` on `{fqn}`", seg.name.name),
        ));
    }

    /// Judges one node-naming `Path` reference. Only multi-segment paths whose
    /// FQN resolves to a *different* module are cross-module; a bare name or a
    /// same-module FQN is the single-module checks' business and is skipped.
    fn check_ref(&mut self, path: &Path, what: &str) {
        let fqn = path_str(path);
        if path.is_simple() {
            return;
        }
        match self.workspace.resolve_qualified(self.from_module, &fqn) {
            Resolution::Public(_) => {}
            Resolution::Private(_) => self.out.push(Diagnostic::error(
                path.span,
                format!("{what} `{fqn}` is private to its module"),
            )),
            Resolution::Missing => {
                // A multi-segment path that names no symbol must be the flat FQN
                // `module::Name` (§8.1, ADR-030). If its leaf names a real,
                // visible node, the path is a structural drill — suggest the flat
                // FQN. Otherwise it is a dangling cross-module reference, or a
                // same-module FQN that resolves nowhere.
                if let Some(flat) = self.flat_fqn_suggestion(&fqn) {
                    self.out.push(Diagnostic::error(
                        path.span,
                        format!("{what} `{fqn}` is not a fully-qualified name; use `{flat}`"),
                    ));
                } else if references_other_module(&fqn, self.from_module, self.workspace) {
                    self.out.push(Diagnostic::error(
                        path.span,
                        format!("dangling cross-module reference `{fqn}`: target does not resolve"),
                    ));
                } else {
                    self.out.push(Diagnostic::error(
                        path.span,
                        format!("{what} `{fqn}` does not resolve"),
                    ));
                }
            }
        }
    }

    /// The flat FQN that a structural drill (`Syntax::Parser` → `syntax::Parser`)
    /// means. A drill's qualifier is a local container/system, so the target is
    /// preferentially in this module; a same-module node bearing the leaf wins.
    /// Failing that, the unique `public` node named by the leaf elsewhere in the
    /// workspace. `None` when neither resolves a single node.
    fn flat_fqn_suggestion(&self, fqn: &str) -> Option<String> {
        let leaf = fqn.rsplit("::").next()?;
        let local = if self.from_module.is_empty() {
            leaf.to_owned()
        } else {
            format!("{}::{leaf}", self.from_module)
        };
        if self.workspace.symbol(&local).is_some() {
            return Some(local);
        }
        let mut matches = self.workspace.symbols().filter(|s| {
            s.name == leaf
                && !self.workspace.is_external_module(module_portion(&s.fqn))
                && s.is_public
        });
        let first = matches.next()?;
        matches.next().is_none().then(|| first.fqn.clone())
    }
}

/// The module portion of an FQN (`a::b::C` → `a::b`); empty for a bare name.
fn module_portion(fqn: &str) -> &str {
    fqn.rsplit_once("::").map_or("", |(module, _)| module)
}

/// Whether `fqn`'s module portion names a known module (local or a dependency,
/// §8.3) other than `from_module` — i.e. the reference points across a module or
/// workspace boundary.
fn references_other_module(fqn: &str, from_module: &str, workspace: &Workspace) -> bool {
    matches!(known_module_prefix(fqn, workspace), Some(module) if module != from_module)
}

/// The longest `::`-delimited prefix of `fqn` that names a known module, if any.
///
/// A node FQN is flat — `module::Name` (§8.1, ADR-030) — so the module is `fqn`'s
/// prefix. A malformed reference may carry extra *structural* segments
/// (`module::System::Container::Component`); the module is still the prefix, and
/// detecting it by the longest match (not the last `::` split) both finds it
/// through the trailing junk and recognises a multi-segment module path
/// (`banking::core`) over its own leading segment.
fn known_module_prefix<'a>(fqn: &'a str, workspace: &Workspace) -> Option<&'a str> {
    let mut module = None;
    let mut end = 0;
    // Walk each `::` boundary left to right; the text before it is a candidate
    // module prefix. The last (longest) one that resolves wins.
    while let Some(rel) = fqn[end..].find("::") {
        let boundary = end + rel;
        if workspace.is_known_module(&fqn[..boundary]) {
            module = Some(&fqn[..boundary]);
        }
        end = boundary + 2;
    }
    module
}

/// Renders a `Path` as its `::`-joined source form.
fn path_str(path: &Path) -> String {
    path.segments
        .iter()
        .map(|id| id.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::model::Workspace;
    use pseudoscript_syntax::parse;

    /// The cross-module error messages for a workspace of `(fqn, source)` modules.
    fn errors(modules: &[(&str, &str)]) -> Vec<String> {
        let ws = Workspace::build(
            modules
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
        );
        check(&ws).iter().map(|d| d.message.clone()).collect()
    }

    // §3.5 / ADR-032: a fieldless variant is referenced `module::Union::Variant`.

    #[test]
    fn unknown_fieldless_variant_is_rejected() {
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic data E =\n  | NotConfigured\n  | Missing\n",
            ),
            (
                "b",
                "//! b\n\npublic system Sys;\npublic container Box for b::Sys {\n  go(): Result<string, a::E> { return Err(a::E::Phantom) }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter()
                .any(|m| m == "`a::E` has no fieldless variant `Phantom`"),
            "{msgs:?}"
        );
    }

    #[test]
    fn fieldless_variant_reference_resolves() {
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic data E =\n  | NotConfigured\n  | Missing\n",
            ),
            (
                "b",
                "//! b\n\npublic system Sys;\npublic container Box for b::Sys {\n  go(): Result<string, a::E> { return Err(a::E::Missing) }\n}\n",
            ),
        ]);
        assert!(msgs.is_empty(), "{msgs:?}");
    }

    #[test]
    fn two_segment_fieldless_variant_form_is_rejected() {
        // A fieldless variant does not hoist, so the two-segment `module::Name`
        // form names no symbol and no union (ADR-032).
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic data E =\n  | NotConfigured\n  | Missing\n",
            ),
            (
                "b",
                "//! b\n\npublic system Sys;\npublic container Box for b::Sys {\n  go(): Result<string, a::E> { return Err(a::Missing) }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter()
                .any(|m| m == "variant reference `a::Missing` does not resolve: no union `a`"),
            "{msgs:?}"
        );
    }

    #[test]
    fn unknown_field_on_cross_module_node_is_rejected() {
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic data Cfg { url: string }\npublic system Sys;\npublic container Box for a::Sys {\n  cfg(): a::Cfg { return self.cfg() }\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Box.ghost }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter().any(|m| m == "no field `ghost` on `a::Box`"),
            "{msgs:?}"
        );
    }

    #[test]
    fn record_variant_value_reference_is_not_a_value() {
        // A record variant hoists to `module::Name`, but as a bare value it is
        // not one — it has fields, so `from` produces it (ADR-035). A fieldless
        // variant referenced through its union stays a value.
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic data Created { id: string }\npublic data E =\n  | Created\n  | Missing\n",
            ),
            (
                "b",
                "//! b\n\npublic system Sys;\npublic container Box for b::Sys {\n  go(): Result<string, a::E> { return Err(a::Created) }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter()
                .any(|m| m == "`Created` is not a value: compose it with `from`"),
            "{msgs:?}"
        );
    }

    // §6 / ADR-022: the first segment of a chain on a node FQN names a member.

    #[test]
    fn unknown_method_on_cross_module_node_is_rejected() {
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic system Sys;\npublic container Box for a::Sys {\n  run(): void {}\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Box.ghost() }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter().any(|m| m == "no method `ghost` on `a::Box`"),
            "{msgs:?}"
        );
    }

    #[test]
    fn known_method_on_cross_module_node_resolves() {
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic system Sys;\npublic container Box for a::Sys {\n  run(): void {}\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Box.run() }\n}\n",
            ),
        ]);
        assert!(msgs.is_empty(), "{msgs:?}");
    }

    #[test]
    fn call_on_black_box_node_is_not_judged() {
        // A black-box node (`;`) discloses no members; absence cannot be proven.
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic system Sys;\npublic container Box for a::Sys;\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Box.run() }\n}\n",
            ),
        ]);
        assert!(msgs.is_empty(), "{msgs:?}");
    }

    // §8.1 (ADR-030): a node FQN is flat (`module::Name`). A C4-structural drill
    // path (`module::System::Container::Component`) names no node; its leaf does,
    // so the checker rejects the drill and suggests the flat FQN.

    #[test]
    fn nested_structural_call_target_suggests_flat_fqn() {
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic system Sys;\npublic container Box for a::Sys;\npublic component Comp for a::Box {\n  run(): void;\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Sys::Box::Comp.run() }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter().any(|m| m
                == "call target `a::Sys::Box::Comp` is not a fully-qualified name; use `a::Comp`"),
            "{msgs:?}"
        );
    }

    #[test]
    fn same_module_structural_drill_suggests_flat_fqn() {
        // The same-module twin: a container::component drill within one module is
        // no longer silent — it suggests the flat FQN.
        let msgs = errors(&[(
            "shop",
            "//! shop\n\npublic system App;\npublic container Box for shop::App;\npublic component Repo for shop::Box {\n  run(): void;\n}\npublic container Caller for shop::App {\n  go(): void { Box::Repo.run() }\n}\n",
        )]);
        assert!(
            msgs.iter()
                .any(|m| m
                    == "call target `Box::Repo` is not a fully-qualified name; use `shop::Repo`"),
            "{msgs:?}"
        );
    }

    #[test]
    fn flat_cross_module_component_call_resolves() {
        // The flat FQN of the same component resolves — the check must not flag it.
        let msgs = errors(&[
            (
                "a",
                "//! a\n\npublic system Sys;\npublic container Box for a::Sys;\npublic component Comp for a::Box {\n  run(): void;\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Comp.run() }\n}\n",
            ),
        ]);
        assert!(msgs.is_empty(), "{msgs:?}");
    }

    #[test]
    fn dangling_multi_segment_reference_does_not_resolve() {
        // A multi-segment path whose leaf names no node is flagged, not silent.
        let msgs = errors(&[
            ("a", "//! a\n\npublic system Sys;\n"),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Sys::Ghost.run() }\n}\n",
            ),
        ]);
        assert!(
            msgs.iter()
                .any(|m| m
                    == "dangling cross-module reference `a::Sys::Ghost`: target does not resolve"),
            "{msgs:?}"
        );
    }
}
