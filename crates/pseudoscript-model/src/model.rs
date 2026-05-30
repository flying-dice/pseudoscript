//! The resolved model: a symbol table over one module's declarations.
//!
//! Static analysis ([`crate::analyze`]) builds a [`Model`] first, then runs the
//! well-formedness checks against it. The LSP can reuse the same table for
//! hover and go-to-definition, so it is part of the public surface.

use pseudoscript_syntax::Span;
use pseudoscript_syntax::ast::{
    Callable, Decl, DeclKind, Field, Item, Module, Node, NodeKind, Param, Type, Variant,
};
use rustc_hash::FxHashMap;

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
}

/// A file-local `alias` binding: the target node FQN it expands to (§8.3).
#[derive(Debug, Clone)]
pub struct Alias {
    /// The `::`-joined target path as written.
    pub target: String,
    /// Span of the alias name's declaration.
    pub span: Span,
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
/// FQNs are file-derived (`LANG.md` §8): a single `.pds` file is one module,
/// named by its `//!` inner-doc path. Every declared node — and every hoisted
/// inline union variant (ADR-006) — lives in this module's namespace.
#[derive(Debug, Clone, Default)]
pub struct Model {
    /// This module's FQN (`banking::core`), or empty for an anonymous module.
    pub module_path: String,
    /// Declared symbols, keyed by bare name (last declaration wins on collision;
    /// collisions are reported separately).
    symbols: FxHashMap<String, Symbol>,
    /// Members (callables, fields) keyed by their owner's bare name.
    members: FxHashMap<String, Vec<Member>>,
    /// `alias` bindings, keyed by the alias name (§8.3).
    aliases: FxHashMap<String, Alias>,
}

impl Model {
    /// Builds the symbol table from a parsed module, deriving the module FQN
    /// from its `//!` inner doc (§8.1).
    #[must_use]
    pub fn build(module: &Module) -> Self {
        Model::build_with_path(module, module_path(module))
    }

    /// Builds the symbol table with a caller-supplied module FQN.
    ///
    /// The workspace loader derives the FQN from the file path relative to
    /// `pds.toml`; [`Model::build`] derives it from the `//!` inner doc.
    #[must_use]
    pub fn build_with_path(module: &Module, module_path: String) -> Self {
        let mut model = Model {
            module_path,
            symbols: FxHashMap::default(),
            members: FxHashMap::default(),
            aliases: FxHashMap::default(),
        };
        for item in &module.items {
            match item {
                Item::Decl(decl) => model.collect_decl(decl),
                Item::Alias(alias) => {
                    model.aliases.insert(
                        alias.name.name.clone(),
                        Alias {
                            target: path_str(&alias.target),
                            span: alias.name.span,
                        },
                    );
                }
                // A `feature` adds no symbol, member, or alias; its name lives in
                // a separate namespace checked in `crate::check` (§5.2, §8.1).
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

    /// Looks up an `alias` by name (§8.3).
    #[must_use]
    pub fn alias(&self, name: &str) -> Option<&Alias> {
        self.aliases.get(name)
    }

    /// Every `alias` name declared in this module.
    pub fn aliases(&self) -> impl Iterator<Item = (&str, &Alias)> {
        self.aliases.iter().map(|(name, a)| (name.as_str(), a))
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
                        for variant in variants {
                            // A hoisted variant shares the union's visibility (§8.2).
                            self.collect_variant(variant, is_public);
                        }
                    }
                    pseudoscript_syntax::ast::DataBody::Record(fields) => {
                        let members = fields.iter().map(field_member).collect();
                        self.members.insert(data.name.name.clone(), members);
                    }
                    pseudoscript_syntax::ast::DataBody::BlackBox => {}
                }
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
        let fqn = if self.module_path.is_empty() {
            name.to_owned()
        } else {
            format!("{}::{name}", self.module_path)
        };
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
    /// Every declared symbol across all modules, keyed by its full FQN.
    by_fqn: FxHashMap<String, Symbol>,
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
        let mut workspace = Workspace::default();
        for (fqn, ast) in modules {
            let model = Model::build_with_path(&ast, fqn.clone());
            for symbol in model.symbols.values() {
                workspace.by_fqn.insert(symbol.fqn.clone(), symbol.clone());
            }
            workspace.modules.push(ModuleEntry { fqn, ast, model });
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

/// Builds a [`Member`] for a node callable, with a one-line signature detail.
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
fn render_type(ty: &Type) -> String {
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

/// Extracts the module FQN from the first `//!` inner-doc line: the first
/// whitespace-delimited token (`//! banking::core — notes` → `banking::core`).
fn module_path(module: &Module) -> String {
    module
        .inner_docs
        .first()
        .and_then(|doc| doc.text.split_whitespace().next())
        .unwrap_or("")
        .to_owned()
}
