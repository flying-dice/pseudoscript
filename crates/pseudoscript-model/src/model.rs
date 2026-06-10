//! The resolved model: a symbol table over one module's declarations.
//!
//! Static analysis ([`crate::analyze`]) builds a [`Model`] first, then runs the
//! well-formedness checks against it. The LSP can reuse the same table for
//! hover and go-to-definition, so it is part of the public surface.

use pseudoscript_syntax::Span;
use pseudoscript_syntax::ast::{
    Callable, Decl, DeclKind, Field, Item, Literal, Module, Node, NodeKind, Param, Type, Variant,
};
use rustc_hash::{FxHashMap, FxHashSet};

/// What kind of declaration a symbol names.
///
/// Mirrors the structural keyword (or `data`); used by the parent-kind and
/// macro-target checks, and rendered as the lowercase word those diagnostics
/// embed (see [`SymbolKind::keyword`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// `person`.
    Person,
    /// `system`.
    System,
    /// `container`.
    Container,
    /// `component`.
    Component,
    /// `data` (top-level or a hoisted inline union variant).
    Data,
    /// `constant` — a top-level primitive value (§3.6, ADR-039).
    Constant,
}

impl SymbolKind {
    /// The lowercase keyword used in diagnostic prose (`container`, `data`, …).
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            SymbolKind::Person => "person",
            SymbolKind::System => "system",
            SymbolKind::Container => "container",
            SymbolKind::Component => "component",
            SymbolKind::Data => "data",
            SymbolKind::Constant => "constant",
        }
    }

    fn from_node_kind(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Person => SymbolKind::Person,
            NodeKind::System => SymbolKind::System,
            NodeKind::Container => SymbolKind::Container,
            NodeKind::Component => SymbolKind::Component,
        }
    }
}

/// What kind of member a [`Member`] is — a node's callable, or a record field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// A callable (operation) declared in a node body (§5.1).
    Callable,
    /// A field of a `data` record (§3.4).
    Field,
}

/// A member reachable through `.` from a node or `data` owner: a callable or a
/// record field. Powers member completion and member go-to-definition.
#[derive(Debug, Clone)]
pub struct Member {
    /// The member's name.
    pub name: String,
    /// Whether it is a callable or a field.
    pub kind: MemberKind,
    /// Span of the member's name (its definition site).
    pub span: Span,
    /// A one-line signature for completion detail (`run(name: string): uuid`).
    pub detail: String,
    /// The member's value type: a field's declared type, or a callable's
    /// return type (`void` when absent). Drives local-binding type inference.
    pub ty: String,
    /// A callable's parameter types, in order (rendered like `ty`); empty for a
    /// field. The arity is `param_types.len()`.
    pub param_types: Vec<String>,
    /// The `///` summary of a callable member, for hover. Fields carry no doc
    /// (the grammar has no field doc), so this is `None` for them.
    pub doc: Option<String>,
    /// Whether the member is `public` (§8.2) — callable-only; a field is always
    /// reachable once its `data` type is (a private type is unreferenceable
    /// cross-module). Gates cross-module member completion.
    pub is_public: bool,
}

/// One declared, addressable node in the module namespace.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// The bare declared name (`Ledger`).
    pub name: String,
    /// The fully-qualified name (`module::Ledger`).
    pub fqn: String,
    /// What kind of declaration introduced it.
    pub kind: SymbolKind,
    /// Whether the declaration is `public` — cross-module addressable (§8.2).
    pub is_public: bool,
    /// Span of the declaration's name.
    pub span: Span,
}

/// The resolved symbol table for one module.
///
/// FQNs are file-derived (`LANG.md` §8.1): a single `.pds` file is one module,
/// named by its file path relative to `pds.toml`. Every declared node — and
/// every hoisted inline union variant (ADR-006) — lives in this module's
/// namespace.
#[derive(Debug, Clone, Default)]
pub struct Model {
    /// This module's FQN (`banking::core`), or empty for an anonymous module.
    pub module_path: String,
    /// Declared symbols, keyed by bare name (last declaration wins on collision;
    /// collisions are reported separately).
    symbols: FxHashMap<String, Symbol>,
    /// Members (callables, fields) keyed by their owner's bare name.
    members: FxHashMap<String, Vec<Member>>,
    /// Each union's fieldless variant names, keyed by the union's bare name
    /// (§3.5). Fieldless variants do not hoist to `symbols`; they are addressed
    /// `module::Union::Variant` (ADR-032), so they index under their union.
    union_variants: FxHashMap<String, Vec<String>>,
    /// Each `constant`'s declared primitive type (`number`/`string`/`bool`),
    /// keyed by its FQN (§3.6, ADR-039). Lets inference resolve a `module::NAME`
    /// reference to its primitive type.
    constant_types: FxHashMap<String, String>,
}

impl Model {
    /// Builds the symbol table for a single, *anonymous* module (no module FQN).
    ///
    /// A module's identity is its file path relative to `pds.toml` (§8.1); the
    /// path-less single-file path (`check`/`eval`, an editor snippet) has no
    /// filename, so it carries no module name. Same-module references resolve;
    /// cross-module resolution needs the path-keyed [`Workspace`].
    #[must_use]
    pub fn build(module: &Module) -> Self {
        Model::build_with_path(module, String::new())
    }

    /// Builds the symbol table with a caller-supplied module FQN.
    ///
    /// The workspace loader derives the FQN from the file path relative to
    /// `pds.toml` (§8.1).
    #[must_use]
    pub fn build_with_path(module: &Module, module_path: String) -> Self {
        let mut model = Model {
            module_path,
            symbols: FxHashMap::default(),
            members: FxHashMap::default(),
            union_variants: FxHashMap::default(),
            constant_types: FxHashMap::default(),
        };
        for item in &module.items {
            match item {
                Item::Decl(decl) => model.collect_decl(decl),
                // A `feature` adds no symbol or member; its name lives in a
                // separate namespace checked in `crate::check` (§5.2, §8.1).
                Item::Feature(_) => {}
            }
        }
        model
    }

    /// Looks up a symbol by its bare name.
    #[must_use]
    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }

    /// Every declared symbol in this module, in unspecified order.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    /// Whether `union` (a bare data name in this module) declares the fieldless
    /// variant `variant` (§3.5, ADR-032). Fieldless variants do not hoist, so
    /// they are not in [`Self::symbol`].
    #[must_use]
    pub fn has_fieldless_variant(&self, union: &str, variant: &str) -> bool {
        self.union_variants
            .get(union)
            .is_some_and(|vs| vs.iter().any(|v| v == variant))
    }

    /// Each `constant`'s FQN paired with its declared primitive type name
    /// (§3.6, ADR-039), in unspecified order.
    pub fn constant_types(&self) -> impl Iterator<Item = (&str, &str)> {
        self.constant_types
            .iter()
            .map(|(fqn, ty)| (fqn.as_str(), ty.as_str()))
    }

    /// The members (callables, fields) of the node or record named `owner`.
    #[must_use]
    pub fn members(&self, owner: &str) -> &[Member] {
        self.members.get(owner).map_or(&[], Vec::as_slice)
    }

    /// The member whose declaration name occupies `span`, with its owner's name.
    /// Lets the LSP resolve a click on a member's *declaration* (a callable or
    /// field name) — members are not in the symbol table, so this is the only
    /// way a bare member name resolves.
    #[must_use]
    pub fn member_at(&self, span: Span) -> Option<(&str, &Member)> {
        self.members.iter().find_map(|(owner, members)| {
            members
                .iter()
                .find(|m| m.span == span)
                .map(|m| (owner.as_str(), m))
        })
    }

    /// Whether `path` (an FQN or bare name) resolves to a declared node in this
    /// module.
    ///
    /// A reference resolves when its final segment names a declared symbol; the
    /// single-module model cannot see other files, so a leading module prefix is
    /// accepted as long as the leaf matches.
    #[must_use]
    pub fn resolves_node(&self, path: &str) -> bool {
        let leaf = path.rsplit("::").next().unwrap_or(path);
        self.symbols.contains_key(leaf)
    }

    /// Whether `path` names this module (or a strict prefix of it) — i.e. a
    /// namespace, not a node (`LANG.md` §8.3).
    #[must_use]
    pub fn is_module_path(&self, path: &str) -> bool {
        path == self.module_path
    }

    fn collect_decl(&mut self, decl: &Decl) {
        let is_public = decl.is_public;
        match &decl.kind {
            DeclKind::Person(node)
            | DeclKind::System(node)
            | DeclKind::Container(node)
            | DeclKind::Component(node) => self.collect_node(node, is_public),
            DeclKind::Data(data) => {
                self.insert(&data.name.name, SymbolKind::Data, is_public, data.name.span);
                match &data.body {
                    pseudoscript_syntax::ast::DataBody::Union(variants) => {
                        let mut fieldless = Vec::new();
                        for variant in variants {
                            // A hoisted record variant shares the union's visibility (§8.2).
                            self.collect_variant(variant, is_public);
                            if variant.record.is_none() {
                                fieldless.push(variant.name.name.clone());
                            }
                        }
                        if !fieldless.is_empty() {
                            self.union_variants
                                .insert(data.name.name.clone(), fieldless);
                        }
                    }
                    pseudoscript_syntax::ast::DataBody::Record(fields) => {
                        let members = fields.iter().map(field_member).collect();
                        self.members.insert(data.name.name.clone(), members);
                    }
                    pseudoscript_syntax::ast::DataBody::BlackBox => {}
                }
            }
            DeclKind::Constant(constant) => {
                // A constant occupies the module's value namespace (§8.1, ADR-039).
                self.insert(
                    &constant.name.name,
                    SymbolKind::Constant,
                    is_public,
                    constant.name.span,
                );
                let fqn = qualify(&self.module_path, &constant.name.name);
                self.constant_types
                    .insert(fqn, literal_prim_name(&constant.value).to_owned());
            }
        }
    }

    fn collect_node(&mut self, node: &Node, is_public: bool) {
        self.insert(
            &node.name.name,
            SymbolKind::from_node_kind(node.kind),
            is_public,
            node.name.span,
        );
        let mut members = Vec::new();
        if let Some(body) = &node.body {
            for member in body {
                match member {
                    pseudoscript_syntax::ast::BodyMember::Decl(decl) => self.collect_decl(decl),
                    pseudoscript_syntax::ast::BodyMember::Callable(callable) => {
                        members.push(callable_member(callable));
                    }
                }
            }
        }
        if !members.is_empty() {
            self.members.insert(node.name.name.clone(), members);
        }
    }

    fn collect_variant(&mut self, variant: &Variant, is_public: bool) {
        // Only inline-declared variants (`| Name { ... }`) hoist a new data
        // symbol; a bare `| Name` references an existing one (ADR-006).
        if variant.record.is_some() {
            self.insert(
                &variant.name.name,
                SymbolKind::Data,
                is_public,
                variant.name.span,
            );
        }
    }

    fn insert(&mut self, name: &str, kind: SymbolKind, is_public: bool, span: Span) {
        let fqn = qualify(&self.module_path, name);
        self.symbols.insert(
            name.to_owned(),
            Symbol {
                name: name.to_owned(),
                fqn,
                kind,
                is_public,
                span,
            },
        );
    }
}

/// One module's parsed AST, resolved [`Model`], and FQN, as held by a
/// [`Workspace`].
///
/// The FQN is caller-supplied (the loader derives it from the file path
/// relative to `pds.toml`, §8.1). For the single-module `analyze`/`check`
/// surfaces it is derived from the `//!` inner doc, preserving today's
/// behaviour.
#[derive(Debug, Clone)]
pub struct ModuleEntry {
    /// This module's FQN (`banking::core`), or empty for an anonymous module.
    pub fqn: String,
    /// The parsed module.
    pub ast: Module,
    /// The resolved per-module symbol table.
    pub model: Model,
}

/// A resolved set of modules keyed by FQN (`LANG.md` §8).
///
/// Built once per workspace from `(fqn, parsed module)` pairs; it owns the
/// per-module [`Model`]s and a global FQN → [`Symbol`] index for cross-module
/// resolution. Cross-module references resolve only to `public` symbols (§8.2),
/// enforced by [`Workspace::resolve_qualified`].
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// Each module's parsed AST and resolved model, in input order.
    modules: Vec<ModuleEntry>,
    /// Every resolvable symbol, keyed by its full FQN: all symbols of the local
    /// modules, plus the `public` symbols of dependency modules (§8.3),
    /// dependency-name-prefixed by the loader.
    by_fqn: FxHashMap<String, Symbol>,
    /// FQNs of dependency (external) modules — indexed for resolution but not
    /// themselves checked. Lets a dangling reference into a known dependency
    /// module be told apart from a local fully-qualified name (§8.4).
    external_modules: FxHashSet<String>,
    /// The resolved [`ModuleEntry`] (AST + model) of each external module, keyed
    /// by FQN — retained, not just its symbols, so completion, hover, goto, and
    /// inference can reach a dependency node's members and doc summaries
    /// (`dep::module::Node.`). External modules are not in `modules`, so this is
    /// their only member/AST source.
    external_entries: FxHashMap<String, ModuleEntry>,
}

/// The outcome of resolving a cross-module reference (`LANG.md` §8.2).
#[derive(Debug, Clone, Copy)]
pub enum Resolution<'a> {
    /// The FQN names a `public` symbol — resolves.
    Public(&'a Symbol),
    /// The FQN names a symbol that is not `public` — a private node reached
    /// from another module.
    Private(&'a Symbol),
    /// No symbol with that FQN exists in any module.
    Missing,
}

impl Workspace {
    /// Builds the workspace from `(fqn, parsed module)` pairs.
    ///
    /// Each module's [`Model`] is built with its caller-supplied FQN so the
    /// global index keys on the correct cross-module names.
    #[must_use]
    pub fn build(modules: impl IntoIterator<Item = (String, Module)>) -> Self {
        Self::build_with_externals(modules, std::iter::empty())
    }

    /// Builds a workspace from its `local` modules plus `external` dependency
    /// modules (§8.3).
    ///
    /// Local modules are indexed and checked as usual. External modules — the
    /// loader supplies them dependency-name-prefixed (`auth::core`) — are indexed
    /// for resolution but *not* checked: a dependency's internal diagnostics are
    /// not the consumer's. A consumer reference `dep::module::Node` then has the
    /// §8.2 visibility rule enforced unchanged by [`Workspace::resolve_qualified`]
    /// (a private dependency target resolves to `Private` and is rejected).
    #[must_use]
    #[tracing::instrument(level = "debug", name = "workspace_build", skip_all)]
    pub fn build_with_externals(
        local: impl IntoIterator<Item = (String, Module)>,
        external: impl IntoIterator<Item = (String, Module)>,
    ) -> Self {
        let mut workspace = Workspace::default();
        for (fqn, ast) in local {
            let model = Model::build_with_path(&ast, fqn.clone());
            for symbol in model.symbols.values() {
                workspace.by_fqn.insert(symbol.fqn.clone(), symbol.clone());
            }
            workspace.modules.push(ModuleEntry { fqn, ast, model });
        }
        for (fqn, ast) in external {
            let model = Model::build_with_path(&ast, fqn.clone());
            // Index every external symbol, public or not: visibility is enforced
            // by `resolve_qualified` (a private target resolves to `Private` and
            // is rejected), and indexing privates lets a reference to one be
            // reported as private rather than dangling (§8.3).
            for symbol in model.symbols.values() {
                workspace.by_fqn.insert(symbol.fqn.clone(), symbol.clone());
            }
            workspace.external_modules.insert(fqn.clone());
            let entry = ModuleEntry {
                fqn: fqn.clone(),
                ast,
                model,
            };
            workspace.external_entries.insert(fqn, entry);
        }
        workspace
    }

    /// The modules in this workspace, in input order.
    #[must_use]
    pub fn modules(&self) -> &[ModuleEntry] {
        &self.modules
    }

    /// Looks up a symbol by its full FQN, regardless of visibility.
    #[must_use]
    pub fn symbol(&self, fqn: &str) -> Option<&Symbol> {
        self.by_fqn.get(fqn)
    }

    /// Every declared symbol across all modules, in unspecified order.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.by_fqn.values()
    }

    /// The module entry named `fqn`, if present.
    #[must_use]
    pub fn module(&self, fqn: &str) -> Option<&ModuleEntry> {
        self.modules.iter().find(|m| m.fqn == fqn)
    }

    /// The module entry named `fqn`, local **or external** (§8.3). Unlike
    /// [`Self::module`] this also finds dependency modules, so hover and goto can
    /// reach a `dep::module::Node`'s declaration and doc summary.
    #[must_use]
    pub fn module_any(&self, fqn: &str) -> Option<&ModuleEntry> {
        self.modules
            .iter()
            .find(|m| m.fqn == fqn)
            .or_else(|| self.external_entries.get(fqn))
    }

    /// The resolved [`Model`] of module `fqn`, local or external (§8.3) — the
    /// member/symbol source for completion and inference into a dependency node.
    #[must_use]
    pub fn module_model(&self, fqn: &str) -> Option<&Model> {
        self.module_any(fqn).map(|entry| &entry.model)
    }

    /// The FQNs of the indexed dependency (external) modules (§8.3), in
    /// unspecified order — the modules a cross-workspace reference can name but
    /// which carry no local source. Completion offers these alongside local
    /// modules as cross-module reference starters.
    pub fn external_module_fqns(&self) -> impl Iterator<Item = &str> {
        self.external_modules.iter().map(String::as_str)
    }

    /// Whether `module_fqn` names a dependency (external) module (§8.3) — one
    /// indexed for resolution but with no local source. A bare cross-workspace
    /// reference is rejected (§8.3 requires `dep::module::Node`), so the
    /// bare-name goto/reference leniency excludes these.
    #[must_use]
    pub fn is_external_module(&self, module_fqn: &str) -> bool {
        self.external_modules.contains(module_fqn)
    }

    /// Whether `module_fqn` names a known module — a local one or an indexed
    /// dependency module (§8.3). Distinguishes a dangling reference into a known
    /// module from a local fully-qualified name the single-module checks own.
    #[must_use]
    pub fn is_known_module(&self, module_fqn: &str) -> bool {
        self.modules.iter().any(|m| m.fqn == module_fqn)
            || self.external_modules.contains(module_fqn)
    }

    /// Resolves a fully-qualified reference made *from* `from_module`, applying
    /// the §8.2 visibility rule: a same-module target resolves regardless of
    /// visibility; a cross-module target resolves only if it is `public`.
    #[must_use]
    pub fn resolve_qualified(&self, from_module: &str, fqn: &str) -> Resolution<'_> {
        let Some(symbol) = self.by_fqn.get(fqn) else {
            return Resolution::Missing;
        };
        let same_module = symbol_module(fqn) == from_module;
        if same_module || symbol.is_public {
            Resolution::Public(symbol)
        } else {
            Resolution::Private(symbol)
        }
    }
}

/// The module portion of a symbol FQN: everything before the final `::`
/// segment (`banking::core::Ledger` → `banking::core`). A bare name has no
/// module (empty string).
fn symbol_module(fqn: &str) -> &str {
    fqn.rsplit_once("::").map_or("", |(module, _)| module)
}

/// A symbol's FQN: `module::name`, or the bare name for an anonymous module —
/// the shared §8.1 rule every symbol insertion follows.
fn qualify(module_path: &str, name: &str) -> String {
    if module_path.is_empty() {
        name.to_owned()
    } else {
        format!("{module_path}::{name}")
    }
}

/// Builds a [`Member`] for a node callable, with a one-line signature detail.
/// The primitive type name a constant literal carries (§3.6, ADR-039).
fn literal_prim_name(literal: &Literal) -> &'static str {
    match literal {
        Literal::String { .. } => "string",
        Literal::Number { .. } => "number",
        Literal::Bool { .. } => "bool",
    }
}

fn callable_member(callable: &Callable) -> Member {
    Member {
        name: callable.name.name.clone(),
        kind: MemberKind::Callable,
        span: callable.name.span,
        detail: callable_detail(callable),
        ty: callable
            .return_ty
            .as_ref()
            .map_or_else(|| "void".to_owned(), render_type),
        param_types: callable.params.iter().map(|p| render_type(&p.ty)).collect(),
        doc: (!callable.doc.summary.is_empty()).then(|| callable.doc.summary.join(" ")),
        is_public: callable.is_public,
    }
}

/// Builds a [`Member`] for a record field (`id: uuid`).
fn field_member(field: &Field) -> Member {
    Member {
        name: field.name.name.clone(),
        kind: MemberKind::Field,
        span: field.name.span,
        detail: format!("{}: {}", field.name.name, render_type(&field.ty)),
        ty: render_type(&field.ty),
        param_types: Vec::new(),
        doc: None,
        is_public: true,
    }
}

/// Renders a callable signature for completion detail: `run(name: string): uuid`.
/// A `void` (absent) return type is omitted.
fn callable_detail(callable: &Callable) -> String {
    let params = callable
        .params
        .iter()
        .map(|p: &Param| format!("{}: {}", p.name.name, render_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let ret = callable
        .return_ty
        .as_ref()
        .map(|ty| format!(": {}", render_type(ty)))
        .unwrap_or_default();
    format!("{}({params}){ret}", callable.name.name)
}

/// Renders a type to its source form: `Result<uuid, string>`, `Account[]`.
pub(crate) fn render_type(ty: &Type) -> String {
    let mut out = path_str(&ty.name);
    if !ty.generics.is_empty() {
        let args = ty
            .generics
            .iter()
            .map(render_type)
            .collect::<Vec<_>>()
            .join(", ");
        out.push('<');
        out.push_str(&args);
        out.push('>');
    }
    if ty.is_array {
        out.push_str("[]");
    }
    out
}

/// Renders a `::`-joined path to its source form.
fn path_str(path: &pseudoscript_syntax::ast::Path) -> String {
    path.segments
        .iter()
        .map(|id| id.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
