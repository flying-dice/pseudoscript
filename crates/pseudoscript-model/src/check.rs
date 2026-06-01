//! The static well-formedness checks (`LANG.md` §3.5, §4, §5.1, §6, §8, §2.4).
//!
//! Each check emits a hand-pinned diagnostic message matching the
//! `CONFORMANCE/static/` goldens. The checks are intentionally conservative:
//! a well-formed model produces zero diagnostics, so each rule fires only when
//! the violation is certain.

use pseudoscript_syntax::ast::{
    Block, BodyMember, Callable, Data, DataBody, Decl, DeclKind, Expr, ExprKind, Feature, Ident,
    Item, Literal, Macro, MacroArg, MacroArgs, MarkerKind, Module, Node, NodeKind, Path, Ref, Stmt,
    StmtKind, Type,
};
use pseudoscript_syntax::{Diagnostic, Span, TokenKind};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::model::{MemberKind, Model, ModuleEntry, SymbolKind, Workspace};

mod cross_module;
mod result_flow;

use result_flow::{Bindings, check_callable_result_flow};

use crate::BUILTIN_MACROS;

/// Runs every static check against a parsed module and its resolved [`Model`].
pub(crate) fn run(module: &Module, model: &Model) -> Vec<Diagnostic> {
    let mut checker = Checker {
        model,
        diagnostics: Vec::new(),
        unions: FxHashMap::default(),
        variant_names: FxHashSet::default(),
    };
    checker.check_module(module);
    checker.diagnostics
}

/// Runs the per-module static checks for every module in a workspace, then the
/// cross-module visibility pass (`LANG.md` §8.2).
pub(crate) fn run_workspace(workspace: &Workspace) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for entry in workspace.modules() {
        diagnostics.extend(run(&entry.ast, &entry.model));
    }
    diagnostics.extend(cross_module::check(workspace));
    diagnostics
}

/// The static diagnostics for one workspace module: its per-module checks plus
/// the cross-module references that originate in it. Every span lies in
/// `entry`'s source. Parse diagnostics are the caller's to prepend.
pub(crate) fn run_module(workspace: &Workspace, entry: &ModuleEntry) -> Vec<Diagnostic> {
    let mut diagnostics = run(&entry.ast, &entry.model);
    diagnostics.extend(cross_module::check_one(workspace, entry));
    diagnostics
}

struct Checker<'a> {
    model: &'a Model,
    diagnostics: Vec<Diagnostic>,
    /// Union type name → its variant names (§3.5), so a return of a variant
    /// satisfies a declared union type.
    unions: FxHashMap<String, FxHashSet<String>>,
    /// Every union variant name (§3.5). A fieldless variant does not hoist to a
    /// symbol, yet is referenced by name to produce its value, so it counts as a
    /// resolvable reference.
    variant_names: FxHashSet<String>,
}

impl Checker<'_> {
    fn check_module(&mut self, module: &Module) {
        self.collect_unions(module);
        self.check_variant_collisions(module);
        self.check_feature_collisions(module);
        self.check_reserved_names(module);
        self.check_type_refs(module);
        for item in &module.items {
            match item {
                Item::Alias(alias) => self.check_alias(alias),
                Item::Decl(decl) => self.check_decl(decl),
                Item::Feature(feature) => self.check_feature(feature),
            }
        }
    }

    /// §2.3 / ADR-012: a declared identifier must not be a reserved word — a
    /// keyword, a primitive type name, or `Result`/`Option`. Covers data, node,
    /// callable, field, parameter, variant, feature, and alias names. (A keyword
    /// in a strict-name position is already a parse error; this catches the
    /// lenient positions and primitives used as any name.)
    fn check_reserved_names(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Decl(decl) => self.check_reserved_decl(&decl.kind),
                Item::Feature(feature) => self.check_reserved_ident(&feature.name),
                Item::Alias(alias) => self.check_reserved_ident(&alias.name),
            }
        }
    }

    fn check_reserved_decl(&mut self, kind: &DeclKind) {
        match kind {
            DeclKind::Data(data) => {
                self.check_reserved_ident(&data.name);
                match &data.body {
                    DataBody::Record(fields) => {
                        for field in fields {
                            self.check_reserved_ident(&field.name);
                        }
                    }
                    DataBody::Union(variants) => {
                        for variant in variants {
                            self.check_reserved_ident(&variant.name);
                            for field in variant.record.iter().flatten() {
                                self.check_reserved_ident(&field.name);
                            }
                        }
                    }
                    DataBody::BlackBox => {}
                }
            }
            DeclKind::Person(node)
            | DeclKind::System(node)
            | DeclKind::Container(node)
            | DeclKind::Component(node) => {
                self.check_reserved_ident(&node.name);
                for member in node.body.iter().flatten() {
                    match member {
                        BodyMember::Callable(callable) => {
                            self.check_reserved_ident(&callable.name);
                            for param in &callable.params {
                                self.check_reserved_ident(&param.name);
                            }
                        }
                        BodyMember::Decl(decl) => self.check_reserved_decl(&decl.kind),
                    }
                }
            }
        }
    }

    fn check_reserved_ident(&mut self, ident: &Ident) {
        if is_reserved(&ident.name) {
            self.error(
                ident.span,
                format!(
                    "reserved word `{}` cannot be used as an identifier",
                    ident.name
                ),
            );
        }
    }

    /// Records each union type's variant names (§3.5), top-level and nested, so a
    /// return of a variant satisfies a declared union type.
    fn collect_unions(&mut self, module: &Module) {
        for item in &module.items {
            if let Item::Decl(decl) = item {
                self.collect_unions_in(&decl.kind);
            }
        }
    }

    fn collect_unions_in(&mut self, kind: &DeclKind) {
        match kind {
            DeclKind::Data(data) => {
                if let DataBody::Union(variants) = &data.body {
                    let names: FxHashSet<String> =
                        variants.iter().map(|v| v.name.name.clone()).collect();
                    self.variant_names.extend(names.iter().cloned());
                    self.unions.insert(data.name.name.clone(), names);
                }
            }
            DeclKind::Person(node)
            | DeclKind::System(node)
            | DeclKind::Container(node)
            | DeclKind::Component(node) => {
                for member in node.body.iter().flatten() {
                    if let BodyMember::Decl(decl) = member {
                        self.collect_unions_in(&decl.kind);
                    }
                }
            }
        }
    }

    /// Whether a determinable return type (`name`, `is_array`) satisfies `ret`:
    /// the array-ness must match and `ret`'s base leaf must equal `name`, or
    /// `name` is a variant of the union `ret` names (§3.5).
    fn type_satisfied(&self, name: &str, is_array: bool, ret: &Type) -> bool {
        if ret.is_array != is_array {
            return false;
        }
        let leaf = path_leaf(&ret.name);
        leaf == name || self.unions.get(leaf).is_some_and(|vs| vs.contains(name))
    }

    // --- §5.2 features ---------------------------------------------------------

    /// §8.1: feature names occupy their own module namespace; a repeat is a
    /// collision.
    fn check_feature_collisions(&mut self, module: &Module) {
        let mut seen: FxHashSet<&str> = FxHashSet::default();
        for item in &module.items {
            if let Item::Feature(feature) = item
                && !seen.insert(&feature.name.name)
            {
                self.error(
                    feature.name.span,
                    format!(
                        "feature `{name}` collides with feature `{name}`",
                        name = feature.name.name
                    ),
                );
            }
        }
    }

    /// §5.2: a feature's `for` target MUST resolve to a node, not a type or
    /// module. A target this single-module model cannot see (another file) is
    /// left to cross-module resolution, mirroring [`Self::check_parent`].
    fn check_feature(&mut self, feature: &Feature) {
        let target = path_str(&feature.target);
        let leaf = target.rsplit("::").next().unwrap_or(&target);
        let is_module = self.model.is_module_path(&target);
        let symbol_kind = self.model.symbol(leaf).map(|s| s.kind);
        let resolves = is_module || symbol_kind.is_some();
        let is_node = !is_module && symbol_kind.is_some_and(|k| k != SymbolKind::Data);
        if resolves && !is_node {
            self.error(
                feature.target.span,
                format!(
                    "feature `{}` target `{target}` is not a node",
                    feature.name.name
                ),
            );
        }
    }

    // --- §8.3 alias resolution -------------------------------------------------

    fn check_alias(&mut self, alias: &pseudoscript_syntax::ast::Alias) {
        let target = path_str(&alias.target);
        if self.model.is_module_path(&target) {
            self.error(
                alias.target.span,
                format!("alias target `{target}` is a module, not a node"),
            );
        } else if !self.model.resolves_node(&target) {
            self.error(
                alias.span,
                format!(
                    "dangling alias `{}`: target `{target}` does not resolve",
                    alias.name.name
                ),
            );
        }
    }

    // --- §3.5 / ADR-006 union variant collision --------------------------------

    fn check_variant_collisions(&mut self, module: &Module) {
        // A top-level `data Name` followed by an inline variant `| Name { ... }`
        // hoisting the same name is a collision. Gather declared data/node names
        // first, then flag any inline variant colliding with a prior data.
        let mut data_names: FxHashSet<&str> = FxHashSet::default();
        for item in &module.items {
            if let Item::Decl(decl) = item {
                self.collect_collisions(&decl.kind, &mut data_names);
            }
        }
    }

    fn collect_collisions<'m>(&mut self, kind: &'m DeclKind, data_names: &mut FxHashSet<&'m str>) {
        match kind {
            DeclKind::Data(data) => self.collect_data_collisions(data, data_names),
            DeclKind::Person(node)
            | DeclKind::System(node)
            | DeclKind::Container(node)
            | DeclKind::Component(node) => {
                if let Some(members) = &node.body {
                    for member in members {
                        if let BodyMember::Decl(decl) = member {
                            self.collect_collisions(&decl.kind, data_names);
                        }
                    }
                }
            }
        }
    }

    fn collect_data_collisions<'m>(&mut self, data: &'m Data, data_names: &mut FxHashSet<&'m str>) {
        data_names.insert(&data.name.name);
        if let DataBody::Union(variants) = &data.body {
            for variant in variants {
                if variant.record.is_some() && !data_names.insert(&variant.name.name) {
                    self.error(
                        variant.span,
                        format!(
                            "variant `{name}` collides with data `{name}`",
                            name = variant.name.name
                        ),
                    );
                }
            }
        }
    }

    // --- declarations ----------------------------------------------------------

    fn check_decl(&mut self, decl: &Decl) {
        match &decl.kind {
            DeclKind::Container(node) => {
                self.check_macros_on_decl(&decl.macros, SymbolKind::Container);
                self.check_parent(node, SymbolKind::System);
                self.check_node_body(node);
            }
            DeclKind::Component(node) => {
                self.check_macros_on_decl(&decl.macros, SymbolKind::Component);
                self.check_parent(node, SymbolKind::Container);
                self.check_node_body(node);
            }
            DeclKind::Person(node) => {
                self.check_macros_on_decl(&decl.macros, SymbolKind::Person);
                self.check_node_body(node);
            }
            DeclKind::System(node) => {
                self.check_macros_on_decl(&decl.macros, SymbolKind::System);
                self.check_node_body(node);
            }
            DeclKind::Data(_) => {
                self.check_macros_on_decl(&decl.macros, SymbolKind::Data);
            }
        }
    }

    fn check_node_body(&mut self, node: &Node) {
        let Some(members) = &node.body else { return };
        for member in members {
            match member {
                BodyMember::Callable(callable) => self.check_callable(callable, &node.name.name),
                BodyMember::Decl(decl) => self.check_decl(decl),
            }
        }
    }

    // --- §4 / ADR-010 parent kind ----------------------------------------------

    fn check_parent(&mut self, node: &Node, required: SymbolKind) {
        let Some(parent) = &node.parent else { return };
        let parent_name = parent.segments.last().map_or("", |id| id.name.as_str());
        // Resolve the parent within this module; a parent we cannot see (another
        // file) is not reportable as a kind error here.
        let Some(symbol) = self.model.symbol(parent_name) else {
            return;
        };
        if symbol.kind != required {
            self.error(
                node.span,
                format!(
                    "{child} `{}` parent `{parent_name}` is not a {req}",
                    node.name.name,
                    child = node_word(node.kind),
                    req = required.keyword(),
                ),
            );
        }
    }

    // --- §2.4 / ADR-015 macros -------------------------------------------------

    fn check_macros_on_decl(&mut self, macros: &[Macro], target: SymbolKind) {
        for mac in macros {
            let name = path_str(&mac.name);
            if BUILTIN_MACROS.contains(&name.as_str()) {
                // Every built-in macro targets callables; on any structural
                // declaration it is a wrong-target error.
                self.error(
                    mac.span,
                    format!("macro `{name}` cannot target a {}", target.keyword()),
                );
            } else {
                self.error(mac.span, format!("unknown macro `{name}`"));
            }
        }
    }

    fn check_callable_macros(&mut self, callable: &Callable) {
        for mac in &callable.macros {
            let name = path_str(&mac.name);
            if !BUILTIN_MACROS.contains(&name.as_str()) {
                self.error(mac.span, format!("unknown macro `{name}`"));
                continue;
            }
            if name == "onevent" {
                self.check_onevent(callable, mac);
            }
        }
    }

    fn check_onevent(&mut self, callable: &Callable, mac: &Macro) {
        let Some(event) = onevent_arg(mac) else {
            return;
        };
        let event_fqn = path_str(event);
        // §2.4: the handler MUST have exactly one parameter whose type equals the
        // event type. Compare on the leaf name; report both FQNs as written.
        let param_ty = match callable.params.as_slice() {
            [only] => &only.ty,
            _ => return,
        };
        if type_leaf(param_ty) != event_fqn.rsplit("::").next().unwrap_or(&event_fqn) {
            let param_fqn = type_str(param_ty);
            self.error(
                callable.span,
                format!(
                    "handler parameter type `{param_fqn}` does not match triggered event `{event_fqn}`"
                ),
            );
        }
    }

    // --- §5.1, §6, §7.1 callable bodies ----------------------------------------

    fn check_callable(&mut self, callable: &Callable, owner: &str) {
        self.check_callable_macros(callable);
        let Some(body) = &callable.body else { return };

        // §7.1 / ADR-002: single-assignment. Seed with parameter names.
        let mut bound: FxHashMap<String, Span> = callable
            .params
            .iter()
            .map(|p| (p.name.name.clone(), p.name.span))
            .collect();
        self.check_rebinds(body, &mut bound);

        // ADR-016: a disclosed non-void callable must return on all paths. An
        // absent return type *and* an explicit `void` both mean void (§5.1).
        let non_void = callable.return_ty.as_ref().is_some_and(|ty| !is_void(ty));
        if non_void && !block_returns(body) {
            self.error(
                callable.span,
                format!(
                    "callable `{}` does not return on all paths",
                    callable.name.name
                ),
            );
        }

        // §6: flow-sensitive Result accessor narrowing.
        let mut env = Bindings::default();
        check_callable_result_flow(body, &mut env, &mut self.diagnostics);

        // §5.1 / §7.2 (ADR-020): a determinable `return` type must match the
        // declared one, and every `from` target must be a `data` record/variant.
        let vars = build_vars(callable, body);
        if let Some(ret) = &callable.return_ty {
            self.check_return_types(body, ret, &vars);
        }
        self.check_from_targets(body);

        // §7.3 / ADR-014: `for` iterates an array.
        let params: FxHashMap<&str, &Type> = callable
            .params
            .iter()
            .map(|p| (p.name.name.as_str(), &p.ty))
            .collect();
        self.check_for_loops(body, &params);

        // §5.1 / ADR-004: a `self.` call names a callable of the enclosing node.
        self.check_self_calls(body, owner);

        // §7/§8: a bare reference resolves to a param, binding, node, or alias.
        self.check_references(callable, body);

        // §2.2/§3.4: a `.field` read on a known `data` record names a field.
        self.check_member_access(body, &vars);

        // §5.1 (ADR-022): a call to a resolvable same-module callable matches its
        // arity.
        self.check_call_arity(body, owner, &vars);

        // §7 (ADR-023): an `if`/`while` condition is boolean.
        self.check_conditions(body, &vars);

        // §7.1: a binding's annotation must match a determinable initialiser.
        self.check_binding_types(body, &vars);
    }

    /// §7.1: a binding states its type (`x: T = expr`). Where the initialiser's
    /// type is determinable — a literal, marker, `from`, or bare reference — it
    /// must match the annotation. Calls, field accesses, `self`, and `::` paths
    /// infer to `Unknown` (ADR-022), so the annotation is authoritative there and
    /// nothing is flagged. A generic annotation (`Result<…>`/`Option<…>`) is
    /// compared only by its constructor, not its inner types.
    fn check_binding_types(&mut self, block: &Block, vars: &FxHashMap<String, Ty>) {
        for_each_stmt(block, &mut |stmt| {
            let StmtKind::Assign { name, ty, value } = &stmt.kind else {
                return;
            };
            // The placeholder type of an untyped-assignment parse error names no
            // type; the missing-annotation diagnostic already stands.
            if !ty.generics.is_empty() || type_leaf(ty).is_empty() {
                return;
            }
            let found = infer(value, vars);
            if !arg_matches(&found, type_leaf(ty), ty.is_array, &self.unions) {
                self.error(
                    value.span,
                    format!(
                        "binding `{}` is annotated `{}` but its value is `{}`",
                        name.name,
                        type_display(ty),
                        ty_display(&found)
                    ),
                );
            }
        });
    }

    /// §7: an `if`/`while` condition whose type is inferable must be `bool`.
    /// Accessor/call conditions infer to `Unknown` and are not checked.
    fn check_conditions(&mut self, block: &Block, vars: &FxHashMap<String, Ty>) {
        for_each_stmt(block, &mut |stmt| match &stmt.kind {
            StmtKind::If { cond, .. } | StmtKind::While { cond, .. } => {
                self.check_condition(cond, vars);
            }
            _ => {}
        });
    }

    fn check_condition(&mut self, cond: &Expr, vars: &FxHashMap<String, Ty>) {
        let ty = infer(cond, vars);
        let non_bool = match &ty {
            Ty::Named { name, array } => *array || name != "bool",
            Ty::Result | Ty::Option => true,
            Ty::Unknown => false,
        };
        if non_bool {
            self.error(
                cond.span,
                format!("condition must be `bool`, found `{}`", ty_display(&ty)),
            );
        }
    }

    /// §5.1: a call whose receiver resolves to a same-module node (`self` or a
    /// node name) must pass exactly as many arguments as the callable declares,
    /// and each inferable argument must match the parameter's type. Cross-module
    /// callees are not visible here and are skipped.
    fn check_call_arity(&mut self, block: &Block, owner: &str, vars: &FxHashMap<String, Ty>) {
        for_each_expr(block, &mut |expr| self.check_call_at(expr, owner, vars));
    }

    /// Checks one call's arity and argument types when its receiver resolves to a
    /// same-module callable. Only the first segment is the call on the receiver.
    fn check_call_at(&mut self, expr: &Expr, owner: &str, vars: &FxHashMap<String, Ty>) {
        let ExprKind::Postfix { base, segments } = &expr.kind else {
            return;
        };
        let Some(seg) = segments.first() else { return };
        let Some(args) = &seg.call_args else { return };
        let Some(node) = self.call_receiver_node(base, owner) else {
            return;
        };
        let Some(params) = self
            .model
            .members(&node)
            .iter()
            .find(|m| m.kind == MemberKind::Callable && m.name == seg.name.name)
            .map(|m| m.param_types.clone())
        else {
            return;
        };
        if args.len() == params.len() {
            self.check_arg_types(args, &params, vars);
        } else {
            self.error(
                seg.span,
                format!(
                    "callable `{}` expects {} argument(s), got {}",
                    seg.name.name,
                    params.len(),
                    args.len()
                ),
            );
        }
    }

    /// Compares each inferable argument's type to its parameter type (positional).
    /// Generic parameters (`Result`/`Option`/…) and `Unknown` arguments are skipped.
    fn check_arg_types(&mut self, args: &[Expr], params: &[String], vars: &FxHashMap<String, Ty>) {
        for (i, (arg, param_str)) in args.iter().zip(params).enumerate() {
            let Some((leaf, array)) = param_shape(param_str) else {
                continue;
            };
            let arg_ty = infer(arg, vars);
            if !arg_matches(&arg_ty, leaf, array, &self.unions) {
                self.error(
                    arg.span,
                    format!(
                        "argument {}: expected `{param_str}`, found `{}`",
                        i + 1,
                        ty_display(&arg_ty)
                    ),
                );
            }
        }
    }

    /// The same-module node a call's receiver resolves to: the enclosing node for
    /// `self`, or a node named by the receiver path's leaf. `None` for a value
    /// receiver or a name that is not a node.
    fn call_receiver_node(&self, base: &Expr, owner: &str) -> Option<String> {
        match &base.kind {
            ExprKind::Ref(Ref::SelfNode(_)) => Some(owner.to_owned()),
            ExprKind::Ref(Ref::Path(path)) => {
                let leaf = path.segments.last()?.name.clone();
                self.model
                    .symbol(&leaf)
                    .filter(|s| s.kind != SymbolKind::Data)
                    .map(|_| leaf)
            }
            _ => None,
        }
    }

    /// §2.2/§3.4: a `.field` read whose receiver is a known same-module `data`
    /// record (with disclosed fields) must name one of its fields. Black-box
    /// data, unions, cross-module types, and call/accessor results are not
    /// inferred and are skipped.
    fn check_member_access(&mut self, block: &Block, vars: &FxHashMap<String, Ty>) {
        for_each_expr(block, &mut |expr| self.check_member_at(expr, vars));
    }

    /// Checks one `.field` read: when the receiver infers to a same-module record,
    /// the first segment must name a field. Only the first segment is on the
    /// receiver; deeper segments chain off an intermediate that is not inferred.
    fn check_member_at(&mut self, expr: &Expr, vars: &FxHashMap<String, Ty>) {
        let ExprKind::Postfix { base, segments } = &expr.kind else {
            return;
        };
        if let Some(seg) = segments.first()
            && seg.call_args.is_none()
            && let Ty::Named { name, array: false } = infer(base, vars)
            && self.is_record(&name)
            && !self.has_field(&name, &seg.name.name)
        {
            // A close real field wins (`.values` typo'd as `.value`); only when
            // none is near do we explain a `Result`/`Option` accessor misuse.
            let cands: Vec<&str> = self
                .model
                .members(&name)
                .iter()
                .map(|m| m.name.as_str())
                .collect();
            let suggestion = suggest(&seg.name.name, &cands);
            let hint = if suggestion.is_empty() {
                accessor_hint(&seg.name.name, &name)
            } else {
                suggestion
            };
            self.error(
                seg.span,
                format!("no field `{}` on `{}`{hint}", seg.name.name, name),
            );
        }
    }

    /// Whether `name` is a same-module `data` type with disclosed fields (a
    /// record). Black-box data and unions expose no fields, so they are skipped.
    fn is_record(&self, name: &str) -> bool {
        self.model
            .symbol(name)
            .is_some_and(|s| s.kind == SymbolKind::Data)
            && !self.model.members(name).is_empty()
    }

    fn has_field(&self, owner: &str, field: &str) -> bool {
        self.model.members(owner).iter().any(|m| m.name == field)
    }

    /// §7/§8: every bare single-segment reference in a body must resolve to a
    /// parameter, a binding, a `for` binding, a node, or an alias. Multi-segment
    /// `::` paths are left to cross-module resolution.
    fn check_references(&mut self, callable: &Callable, body: &Block) {
        let mut scope: FxHashSet<&str> = callable
            .params
            .iter()
            .map(|p| p.name.name.as_str())
            .collect();
        collect_bound_names(body, &mut scope);
        for_each_expr(body, &mut |expr| self.check_ref_at(expr, &scope));
    }

    /// Checks one expression: a bare single-segment reference that is not a
    /// parameter, binding, node, alias, or variant is unresolved.
    fn check_ref_at(&mut self, expr: &Expr, scope: &FxHashSet<&str>) {
        let ExprKind::Ref(Ref::Path(path)) = &expr.kind else {
            return;
        };
        if !path.is_simple() {
            return;
        }
        let name = &path.segments[0].name;
        if !scope.contains(name.as_str())
            && self.model.symbol(name).is_none()
            && self.model.alias(name).is_none()
            && !self.variant_names.contains(name.as_str())
        {
            let hint = if name == "void" {
                // `void` is a type, not a value: a void callable returns with a
                // bare `Ok` (or `return`), never `Ok(void)` (§5.1, §6.1).
                "; `void` is a type, not a value — a void result returns bare `Ok` (§6.1)"
                    .to_owned()
            } else {
                let candidates: Vec<&str> = scope
                    .iter()
                    .copied()
                    .chain(self.model.symbols().map(|s| s.name.as_str()))
                    .chain(self.model.aliases().map(|(n, _)| n))
                    .chain(self.variant_names.iter().map(String::as_str))
                    .collect();
                suggest(name, &candidates)
            };
            self.error(path.span, format!("unresolved reference `{name}`{hint}"));
        }
    }

    /// §5.1 / ADR-004: `self.Name(args)` MUST name a callable of the enclosing
    /// node `owner`.
    fn check_self_calls(&mut self, block: &Block, owner: &str) {
        for_each_expr(block, &mut |expr| self.check_self_call_at(expr, owner));
    }

    /// Checks one `self.Method(args)` call: the first segment, when a call
    /// directly on `self`, must name a callable of the enclosing node.
    fn check_self_call_at(&mut self, expr: &Expr, owner: &str) {
        let ExprKind::Postfix { base, segments } = &expr.kind else {
            return;
        };
        if matches!(&base.kind, ExprKind::Ref(Ref::SelfNode(_)))
            && let Some(seg) = segments.first()
            && seg.call_args.is_some()
            && !self.owner_has_callable(owner, &seg.name.name)
        {
            let hint = {
                let cands: Vec<&str> = self
                    .model
                    .members(owner)
                    .iter()
                    .filter(|m| m.kind == MemberKind::Callable)
                    .map(|m| m.name.as_str())
                    .collect();
                suggest(&seg.name.name, &cands)
            };
            self.error(
                seg.span,
                format!(
                    "`self.{}` does not name a callable of `{owner}`{hint}",
                    seg.name.name
                ),
            );
        }
    }

    /// Whether the node `owner` declares a callable named `method`.
    fn owner_has_callable(&self, owner: &str, method: &str) -> bool {
        self.model
            .members(owner)
            .iter()
            .any(|m| m.kind == MemberKind::Callable && m.name == method)
    }

    /// §7.3: `for (x in Expr)` requires `Expr` to be an array. When the
    /// iterable's type is determinable — a parameter, literal, marker, or `from`
    /// — a non-array is rejected. Bindings, calls, and field accesses are left
    /// unchecked.
    fn check_for_loops(&mut self, block: &Block, params: &FxHashMap<&str, &Type>) {
        for_each_stmt(block, &mut |stmt| {
            if let StmtKind::For { iter, .. } = &stmt.kind
                && let Some((name, is_array)) = iter_shape(iter, params)
                && !is_array
            {
                self.error(
                    iter.span,
                    format!("`for` iterates a non-array type `{name}`"),
                );
            }
        });
    }

    // --- §5.1 / §7.2 (ADR-020) return-type & `from` checks ---------------------

    /// A `return` whose operand has a statically-known type — a literal, an
    /// `Ok`/`Err`/`Some`/`None` marker, or a `Type from { .. }` composition —
    /// must match the declared return type `ret`. Calls, field accesses, and
    /// `self` yield `Unknown` and are left unchecked.
    fn check_return_types(&mut self, block: &Block, ret: &Type, vars: &FxHashMap<String, Ty>) {
        for_each_stmt(block, &mut |stmt| {
            if let StmtKind::Return(Some(expr)) = &stmt.kind {
                self.check_return_expr(expr, ret, vars);
            }
        });
    }

    fn check_return_expr(&mut self, expr: &Expr, ret: &Type, vars: &FxHashMap<String, Ty>) {
        let ty = infer(expr, vars);
        if matches!(ty, Ty::Unknown) {
            return;
        }
        if !self.ty_satisfies_ret(&ty, ret) {
            self.error(
                expr.span,
                format!(
                    "return type `{}` does not match declared `{}`",
                    ty_display(&ty),
                    type_display(ret)
                ),
            );
        }
    }

    /// Whether an inferred type satisfies the declared return type. `Unknown` is
    /// permissive (not flagged); a `Named` reuses the array/union rule.
    fn ty_satisfies_ret(&self, ty: &Ty, ret: &Type) -> bool {
        match ty {
            Ty::Named { name, array } => self.type_satisfied(name, *array, ret),
            Ty::Result => !ret.is_array && path_leaf(&ret.name) == "Result",
            Ty::Option => !ret.is_array && path_leaf(&ret.name) == "Option",
            Ty::Unknown => true,
        }
    }

    /// §7.2: a `from` target MUST resolve to a `data` record or union variant.
    /// Walks every expression in the body, flagging a `from` whose target is a
    /// primitive, `Result`/`Option`, or a node.
    fn check_from_targets(&mut self, block: &Block) {
        for_each_expr(block, &mut |expr| {
            if let ExprKind::From { ty, .. } = &expr.kind {
                self.check_from_target(ty, expr.span);
            }
        });
    }

    fn check_from_target(&mut self, ty: &Path, span: Span) {
        let leaf = ty.segments.last().map_or("", |id| id.name.as_str());
        let invalid_builtin = is_primitive(leaf) || leaf == "Result" || leaf == "Option";
        // A target this single-module model resolves to a non-`data` symbol is a
        // node; a target it cannot see (another module) is left alone, mirroring
        // `check_parent`.
        let resolves_non_data = self
            .model
            .symbol(leaf)
            .is_some_and(|s| s.kind != SymbolKind::Data);
        if invalid_builtin || resolves_non_data {
            self.error(
                span,
                format!("`from` target `{leaf}` is not a `data` record or variant"),
            );
        }
    }

    // --- §3.1/§3.3/§8 type-reference resolution --------------------------------

    /// §3.3: every named type in a declaration — a field, parameter, or return
    /// type, and each generic argument — must resolve. Walks every type
    /// annotation in the module; [`Self::check_type`] judges each.
    fn check_type_refs(&mut self, module: &Module) {
        for item in &module.items {
            if let Item::Decl(decl) = item {
                self.check_decl_types(&decl.kind);
            }
        }
    }

    fn check_decl_types(&mut self, kind: &DeclKind) {
        match kind {
            DeclKind::Data(data) => self.check_data_field_types(&data.body),
            DeclKind::Person(node)
            | DeclKind::System(node)
            | DeclKind::Container(node)
            | DeclKind::Component(node) => {
                for member in node.body.iter().flatten() {
                    match member {
                        BodyMember::Callable(callable) => {
                            for param in &callable.params {
                                self.check_type(&param.ty);
                            }
                            if let Some(ret) = &callable.return_ty {
                                self.check_type(ret);
                            }
                        }
                        BodyMember::Decl(decl) => self.check_decl_types(&decl.kind),
                    }
                }
            }
        }
    }

    /// Checks the field types of a `data` record and of each record variant of a
    /// union (§3.4, §3.5). A black box and a bare variant disclose no fields.
    fn check_data_field_types(&mut self, body: &DataBody) {
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

    /// §3.3: checks one type annotation's base name and every generic argument. A
    /// single-segment base that resolves to nothing is reported; a `::`-qualified
    /// base is left to cross-module resolution (§8.2), mirroring
    /// [`Self::check_ref_at`].
    fn check_type(&mut self, ty: &Type) {
        if ty.name.is_simple() {
            let leaf = ty.name.segments[0].name.as_str();
            if !self.type_resolves(leaf) {
                let hint = {
                    let candidates: Vec<&str> = TokenKind::PRIMITIVE_TYPES
                        .iter()
                        .copied()
                        .chain(["Result", "Option"])
                        .chain(self.model.symbols().map(|s| s.name.as_str()))
                        .chain(self.model.aliases().map(|(n, _)| n))
                        .chain(self.variant_names.iter().map(String::as_str))
                        .collect();
                    suggest(leaf, &candidates)
                };
                self.error(ty.name.span, format!("unresolved type `{leaf}`{hint}"));
            }
        }
        for generic in &ty.generics {
            self.check_type(generic);
        }
    }

    /// Whether a single-segment type name resolves (§3.1/§3.3): a primitive,
    /// `Result`/`Option`, or an in-module symbol, alias, or union variant. A
    /// node names a type as freely as a `data` record does (`owner: Person`).
    fn type_resolves(&self, leaf: &str) -> bool {
        is_primitive(leaf)
            || leaf == "Result"
            || leaf == "Option"
            || self.model.symbol(leaf).is_some()
            || self.model.alias(leaf).is_some()
            || self.variant_names.contains(leaf)
    }

    fn check_rebinds(&mut self, block: &Block, bound: &mut FxHashMap<String, Span>) {
        // No shadowing across nested blocks (ADR-002): every binding shares one
        // function-wide scope, so a flat statement walk suffices.
        for_each_stmt(block, &mut |stmt| match &stmt.kind {
            StmtKind::Assign { name, .. } => self.bind_or_error(name, bound),
            StmtKind::For { binding, .. } => self.bind_or_error(binding, bound),
            _ => {}
        });
    }

    /// Records `ident` as bound, or reports a re-binding if it already is (§7.1).
    fn bind_or_error(&mut self, ident: &Ident, bound: &mut FxHashMap<String, Span>) {
        if bound.contains_key(&ident.name) {
            self.error(
                ident.span,
                format!(
                    "re-binding of `{}`: bindings are single-assignment",
                    ident.name
                ),
            );
        } else {
            bound.insert(ident.name.clone(), ident.span);
        }
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(span, message));
    }
}

/// An inferred expression type (§5.1/§7.2). Conservative: `Unknown` whenever
/// the type is not statically determinable, so a check never fires on a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ty {
    /// A primitive or `data` type, with an array flag.
    Named { name: String, array: bool },
    /// A `Result<…>` value (inner types not tracked).
    Result,
    /// An `Option<…>` value (inner type not tracked).
    Option,
    /// Not inferable.
    Unknown,
}

impl Ty {
    fn named(name: &str) -> Ty {
        Ty::Named {
            name: name.to_owned(),
            array: false,
        }
    }
}

/// Builds the local typing context: parameter types plus each binding's
/// inferred type (single-assignment, function-scoped).
fn build_vars(callable: &Callable, body: &Block) -> FxHashMap<String, Ty> {
    let mut vars: FxHashMap<String, Ty> = callable
        .params
        .iter()
        .map(|p| (p.name.name.clone(), ty_from_ast(&p.ty)))
        .collect();
    collect_binding_types(body, &mut vars);
    vars
}

fn collect_binding_types(block: &Block, vars: &mut FxHashMap<String, Ty>) {
    for_each_stmt(block, &mut |stmt| match &stmt.kind {
        StmtKind::Assign { name, value, .. } => {
            // The result-flow lattice keeps inferring from the RHS (calls stay
            // Unknown, ADR-022); the annotation is validated separately.
            let ty = infer(value, vars);
            vars.insert(name.name.clone(), ty);
        }
        // The element type of the iterated array is not inferred.
        StmtKind::For { binding, .. } => {
            vars.insert(binding.name.clone(), Ty::Unknown);
        }
        _ => {}
    });
}

/// Infers an expression's type where statically determinable: literals,
/// markers, `from`, and bare parameter/binding references. Calls, field
/// accesses, `self`, and `::` paths yield `Unknown` (not inferred).
fn infer(expr: &Expr, vars: &FxHashMap<String, Ty>) -> Ty {
    match &expr.kind {
        ExprKind::Literal(Literal::String { .. }) => Ty::named("string"),
        ExprKind::Literal(Literal::Number { .. }) => Ty::named("number"),
        ExprKind::Literal(Literal::Bool { .. }) | ExprKind::Unary { .. } => Ty::named("bool"),
        ExprKind::Marker { kind, .. } => match kind {
            MarkerKind::Ok | MarkerKind::Err => Ty::Result,
            MarkerKind::Some | MarkerKind::None => Ty::Option,
        },
        ExprKind::From { ty, is_array, .. } => Ty::Named {
            name: path_leaf(ty).to_owned(),
            array: *is_array,
        },
        ExprKind::Ref(Ref::Path(path)) if path.is_simple() => vars
            .get(&path.segments[0].name)
            .cloned()
            .unwrap_or(Ty::Unknown),
        ExprKind::Paren(inner) => infer(inner, vars),
        _ => Ty::Unknown,
    }
}

/// A parameter type as `(leaf, is_array)`, or `None` when it is generic
/// (`Result<…>`/`Option<…>`) — those carry inner types this checker does not
/// compare. The leaf drops any `::` qualifier, matching inferred types which are
/// compared by leaf name.
fn param_shape(rendered: &str) -> Option<(&str, bool)> {
    if rendered.contains('<') {
        return None;
    }
    let (base, array) = match rendered.strip_suffix("[]") {
        Some(base) => (base, true),
        None => (rendered, false),
    };
    Some((base.rsplit("::").next().unwrap_or(base), array))
}

/// Whether an inferred argument type matches a parameter `(leaf, array)`, with a
/// union variant satisfying its union. `Unknown` matches anything.
fn arg_matches(
    arg: &Ty,
    leaf: &str,
    array: bool,
    unions: &FxHashMap<String, FxHashSet<String>>,
) -> bool {
    match arg {
        Ty::Named { name, array: a } => {
            *a == array && (name == leaf || unions.get(leaf).is_some_and(|vs| vs.contains(name)))
        }
        Ty::Result => !array && leaf == "Result",
        Ty::Option => !array && leaf == "Option",
        Ty::Unknown => true,
    }
}

/// The [`Ty`] of an AST type annotation (a parameter/field/return type).
fn ty_from_ast(ty: &Type) -> Ty {
    match path_leaf(&ty.name) {
        "Result" => Ty::Result,
        "Option" => Ty::Option,
        name => Ty::Named {
            name: name.to_owned(),
            array: ty.is_array,
        },
    }
}

/// Renders a [`Ty`] for a diagnostic.
fn ty_display(ty: &Ty) -> String {
    match ty {
        Ty::Named { name, array } if *array => format!("{name}[]"),
        Ty::Named { name, .. } => name.clone(),
        Ty::Result => "Result".to_owned(),
        Ty::Option => "Option".to_owned(),
        Ty::Unknown => "?".to_owned(),
    }
}

/// The `(type name, is_array)` of a `for` iterable when statically determinable
/// — a parameter reference, a literal, a marker, or a `from` composition — else
/// `None` (a binding, call, or field access is not inferred).
fn iter_shape(iter: &Expr, params: &FxHashMap<&str, &Type>) -> Option<(String, bool)> {
    match &iter.kind {
        ExprKind::Ref(Ref::Path(path)) if path.is_simple() => params
            .get(path.segments[0].name.as_str())
            .map(|ty| (path_leaf(&ty.name).to_owned(), ty.is_array)),
        ExprKind::Literal(Literal::String { .. }) => Some(("string".to_owned(), false)),
        ExprKind::Literal(Literal::Number { .. }) => Some(("number".to_owned(), false)),
        ExprKind::Literal(Literal::Bool { .. }) => Some(("bool".to_owned(), false)),
        ExprKind::Marker { kind, .. } => Some((
            match kind {
                MarkerKind::Ok | MarkerKind::Err => "Result",
                MarkerKind::Some | MarkerKind::None => "Option",
            }
            .to_owned(),
            false,
        )),
        ExprKind::From { ty, is_array, .. } => Some((path_leaf(ty).to_owned(), *is_array)),
        ExprKind::Paren(inner) => iter_shape(inner, params),
        _ => None,
    }
}

/// Renders a type for a diagnostic: its base leaf plus `[]` when an array.
fn type_display(ty: &Type) -> String {
    let leaf = path_leaf(&ty.name);
    if ty.is_array {
        format!("{leaf}[]")
    } else {
        leaf.to_owned()
    }
}

/// The final segment of a path, or empty.
fn path_leaf(path: &Path) -> &str {
    path.segments.last().map_or("", |id| id.name.as_str())
}

/// Collects every name a body binds — assignment targets and `for` bindings,
/// across all nested blocks — into `scope` (single-assignment, function-scoped,
/// so order does not matter).
fn collect_bound_names<'a>(block: &'a Block, scope: &mut FxHashSet<&'a str>) {
    for_each_stmt(block, &mut |stmt| match &stmt.kind {
        StmtKind::Assign { name, .. } => {
            scope.insert(&name.name);
        }
        StmtKind::For { binding, .. } => {
            scope.insert(&binding.name);
        }
        _ => {}
    });
}

/// A "did you mean X" hint naming the closest candidate within a small edit
/// distance (rustc-style: distance no more than max length over 3), or empty
/// when none is close enough. Damerau-Levenshtein counts an adjacent
/// transposition (teh vs the) as a single edit.
fn suggest(typed: &str, candidates: &[&str]) -> String {
    candidates
        .iter()
        .map(|c| (strsim::damerau_levenshtein(typed, c), *c))
        .filter(|(d, c)| *d > 0 && *d <= typed.len().max(c.len()) / 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| format!("; did you mean `{c}`?"))
        .unwrap_or_default()
}

/// A hint when `name` is a `Result`/`Option` accessor (§6) read off a value that
/// is neither — the common cause is a field declared as a plain type that should
/// be `Option<T>`. Empty when `name` is an ordinary (mistyped) field.
fn accessor_hint(name: &str, receiver: &str) -> String {
    match name {
        "isOk" | "isErr" | "error" => {
            format!("; `.{name}` is a `Result` accessor (§6.1) — `{receiver}` is not a `Result`")
        }
        "isSome" | "isNone" => {
            format!(
                "; `.{name}` is an `Option` accessor (§6.2) — type the value `Option<{receiver}>` to use it"
            )
        }
        "value" => {
            format!("; `.value` reads a `Result`/`Option` payload (§6) — `{receiver}` is neither")
        }
        _ => String::new(),
    }
}

/// Visits every statement in `block`, descending into nested `if`/`for`/`while`
/// blocks, calling `f` on each. The single statement-recursion skeleton the body
/// checks share.
fn for_each_stmt<'a>(block: &'a Block, f: &mut impl FnMut(&'a Stmt)) {
    for stmt in &block.stmts {
        f(stmt);
        match &stmt.kind {
            StmtKind::If {
                then_block,
                else_block,
                ..
            } => {
                for_each_stmt(then_block, f);
                if let Some(else_block) = else_block {
                    for_each_stmt(else_block, f);
                }
            }
            StmtKind::For { body, .. } | StmtKind::While { body, .. } => for_each_stmt(body, f),
            _ => {}
        }
    }
}

/// Visits `expr` and every sub-expression — a postfix base and its call
/// arguments, `from` sources, a marker payload, a unary/paren operand — calling
/// `f` on each.
fn walk_expr(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match &expr.kind {
        ExprKind::Postfix { base, segments } => {
            walk_expr(base, f);
            for seg in segments {
                for arg in seg.call_args.iter().flatten() {
                    walk_expr(arg, f);
                }
            }
        }
        ExprKind::From { sources, .. } => {
            for src in sources {
                walk_expr(src, f);
            }
        }
        ExprKind::Marker {
            payload: Some(payload),
            ..
        } => walk_expr(payload, f),
        ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) => walk_expr(expr, f),
        _ => {}
    }
}

/// Visits every expression in `block` — each statement's operands and all their
/// sub-expressions — in evaluation order.
fn for_each_expr(block: &Block, f: &mut impl FnMut(&Expr)) {
    for_each_stmt(block, &mut |stmt| match &stmt.kind {
        StmtKind::Assign { value, .. } | StmtKind::Expr(value) => walk_expr(value, f),
        StmtKind::Return(Some(expr)) => walk_expr(expr, f),
        StmtKind::If { cond, .. } | StmtKind::While { cond, .. } => walk_expr(cond, f),
        StmtKind::For { iter, .. } => walk_expr(iter, f),
        StmtKind::Return(None) => {}
    });
}

/// Whether `name` is a primitive type name (§3.1).
fn is_primitive(name: &str) -> bool {
    matches!(
        name,
        "number" | "string" | "bool" | "datetime" | "uuid" | "void"
    )
}

/// Whether `name` is a reserved word (§2.3): a keyword, a primitive type name,
/// or `Result`/`Option`.
fn is_reserved(name: &str) -> bool {
    TokenKind::KEYWORDS.contains(&name)
        || TokenKind::PRIMITIVE_TYPES.contains(&name)
        || name == "Result"
        || name == "Option"
}

/// Whether a block returns on every path (ADR-016).
///
/// A `return` makes a block diverge. An `if`/`else` diverges only when both
/// arms do. `for`/`while` bodies and bare expressions never guarantee a return,
/// so a block ending in anything but a `return` or a both-arms `if`/`else`
/// falls through.
fn block_returns(block: &Block) -> bool {
    block.stmts.iter().any(stmt_returns)
}

fn stmt_returns(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::If {
            then_block,
            else_block: Some(else_block),
            ..
        } => block_returns(then_block) && block_returns(else_block),
        _ => false,
    }
}

fn node_word(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Person => "person",
        NodeKind::System => "system",
        NodeKind::Container => "container",
        NodeKind::Component => "component",
    }
}

/// Renders a `Path` as its `::`-joined source form.
fn path_str(path: &Path) -> String {
    path.segments
        .iter()
        .map(|id| id.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

/// Renders a type's base path as written (ignores generics/array suffix, which
/// the event-match diagnostic does not embed).
fn type_str(ty: &Type) -> String {
    path_str(&ty.name)
}

/// The final segment of a type's base path.
fn type_leaf(ty: &Type) -> &str {
    ty.name.segments.last().map_or("", |id| id.name.as_str())
}

/// Whether a type is the primitive `void` (§3.1): a bare, non-generic,
/// non-array `void`.
fn is_void(ty: &Type) -> bool {
    !ty.is_array
        && ty.generics.is_empty()
        && ty.name.is_simple()
        && ty.name.segments[0].name == "void"
}

/// Extracts the single `Path` argument of an `#[onevent(Event)]` macro.
fn onevent_arg(mac: &Macro) -> Option<&Path> {
    match &mac.args {
        MacroArgs::List(args) => args.iter().find_map(|arg| match arg {
            MacroArg::Path(path) => Some(path),
            _ => None,
        }),
        _ => None,
    }
}
