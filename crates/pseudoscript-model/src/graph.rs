//! The resolved relationship graph (`LANG.md` §9).
//!
//! [`Graph`] is a pure projection of a resolved [`Workspace`]: every structural
//! node and callable as a [`GraphNode`], the typed relationships between them as
//! [`Edge`]s, and — for each disclosed callable — an ordered [`Step`] trace the
//! emit crate lays out as a sequence diagram. It carries exactly the information
//! the `Scene` IR (`CONFORMANCE/generation/README.md`) needs to project the
//! context, container, component, and sequence views.
//!
//! The graph is built once, without I/O, so a later salsa/LSP layer can adopt
//! it (PATTERNS.md). Call/alias/`self.` targets are resolved to FQNs where they
//! resolve; an unresolved target is recorded as-written, not dropped — hard
//! resolution errors are the static checks' job (`crate::check_workspace`).

use pseudoscript_syntax::Span;
use pseudoscript_syntax::ast::{
    self, Block, BodyMember, Callable, Decl, DeclKind, Expr, ExprKind, Feature, Item, Literal,
    Macro, MacroArg, MacroArgs, Node, Path, PostfixSeg, Ref, Stmt, StmtKind,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::model::{ModuleEntry, Workspace};

/// What kind of model element a [`GraphNode`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// `person`.
    Person,
    /// `system`.
    System,
    /// `container`.
    Container,
    /// `component`.
    Component,
    /// `data` (top-level or a hoisted union variant).
    Data,
    /// A callable (implicit operation, §5.1).
    Callable,
}

impl NodeKind {
    /// The lowercase keyword for this kind (`container`, `callable`, …).
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            NodeKind::Person => "person",
            NodeKind::System => "system",
            NodeKind::Container => "container",
            NodeKind::Component => "component",
            NodeKind::Data => "data",
            NodeKind::Callable => "callable",
        }
    }

    fn from_ast(kind: ast::NodeKind) -> Self {
        match kind {
            ast::NodeKind::Person => NodeKind::Person,
            ast::NodeKind::System => NodeKind::System,
            ast::NodeKind::Container => NodeKind::Container,
            ast::NodeKind::Component => NodeKind::Component,
        }
    }
}

/// Whether a node is `public` (cross-module addressable) or module-private
/// (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// `public` — cross-module addressable.
    Public,
    /// Module-private — same-file only.
    Private,
}

impl Visibility {
    fn from_bool(is_public: bool) -> Self {
        if is_public {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }
}

/// A trigger macro on a callable (`LANG.md` §2.4, ADR-015): the macro that
/// makes the callable a diagram entry point and synthesises an inbound
/// initiator edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    /// `#[onevent(Event)]` — initiator `event:<Event-FQN>`.
    OnEvent {
        /// The triggering event type's FQN, as written.
        event_fqn: String,
    },
    /// `#[schedule = "..."]` — initiator `scheduler`.
    Schedule,
    /// `#[http(...)]` — initiator `client`.
    Http,
    /// `#[manual]` — initiator `caller`.
    Manual,
}

impl Trigger {
    /// The synthesised initiator node FQN this trigger contributes an edge from
    /// (`event:<FQN>`, `scheduler`, `client`, `caller`).
    #[must_use]
    pub fn initiator(&self) -> String {
        match self {
            Trigger::OnEvent { event_fqn } => format!("event:{event_fqn}"),
            Trigger::Schedule => "scheduler".to_owned(),
            Trigger::Http => "client".to_owned(),
            Trigger::Manual => "caller".to_owned(),
        }
    }
}

/// A callable's signature, in source form: its parameters and return type
/// (`void` when absent). Populated only for callables; drives the type detail on
/// sequence-diagram messages (`LANG.md` §9.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// The parameters, in declaration order.
    pub params: Vec<SigParam>,
    /// The rendered return type (`Result<Order, Rejected>`, `void`, …).
    pub ret: String,
}

/// One parameter of a [`Signature`]: its name and rendered type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigParam {
    /// The parameter name.
    pub name: String,
    /// The rendered parameter type.
    pub ty: String,
}

/// A `data` declaration's disclosed shape (§3.5): a record of typed fields, a
/// discriminated union of variants, or an undisclosed black box. Populated only
/// for [`NodeKind::Data`] nodes; drives the entity (ER) view (`LANG.md` §9.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "form")]
pub enum DataShape {
    /// `{ field: Type, … }`.
    Record {
        /// The fields, in declaration order.
        fields: Vec<DataField>,
    },
    /// `= | A | B { … }`.
    Union {
        /// The variants, in declaration order.
        variants: Vec<DataVariant>,
    },
    /// `;` — shape undisclosed.
    BlackBox,
}

/// One field of a record (or a variant's inline record): its name and rendered
/// type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataField {
    /// The field name.
    pub name: String,
    /// The rendered field type (`OrderId`, `Result<Order, Rejected>`, …).
    pub ty: String,
}

/// One variant of a union (§3.5): its name and, when it declares an inline
/// record body, that record's fields (`None` for a bare `| Name` reference).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataVariant {
    /// The variant name.
    pub name: String,
    /// The inline record's fields, or `None` for a bare reference variant.
    pub fields: Option<Vec<DataField>>,
}

/// One node in the resolved graph: a structural declaration, a `data` type, or
/// a callable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// The node's fully-qualified name (`banking::core::Ledger`, or for a
    /// callable `banking::core::Ledger::Fetch`).
    pub fqn: String,
    /// The node's simple (final-segment) name.
    pub name: String,
    /// What kind of element it is.
    pub kind: NodeKind,
    /// The FQN of the enclosing node: a `for` parent for container/component,
    /// the owning node for a callable, the containing node for a nested decl.
    /// `None` for a top-level person/system/data.
    pub parent: Option<String>,
    /// The FQN of the module that declares it.
    pub module: String,
    /// Whether it is `public` (§8.2).
    pub visibility: Visibility,
    /// Source span of the declaration's name.
    pub span: Span,
    /// Trigger macros on the node — only ever populated for callables (§2.4).
    pub triggers: Vec<Trigger>,
    /// The callable's signature — `None` for non-callable nodes (§9.2).
    pub signature: Option<Signature>,
    /// The `data` declaration's disclosed shape — `None` for non-`data` nodes
    /// (`LANG.md` §3.5, §9.4).
    pub shape: Option<DataShape>,
    /// Documentation lifted from the declaration's `///` block (§2.1).
    pub doc: NodeDoc,
}

/// A node's documentation, lifted from its `///` block (§2.1, ADR-009): the
/// summary (compact-diagram text), the extended description (tooltip text), and
/// its tags. `LANG.md` §9.1 — `///` summaries become descriptions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDoc {
    /// Summary lines, joined with `\n`; `None` when the block has no summary.
    pub summary: Option<String>,
    /// Extended-description lines, joined with `\n`; `None` when absent.
    pub extended: Option<String>,
    /// Tags (`#name`, including the `#`), in source order.
    pub tags: Vec<String>,
}

/// Lifts a syntax [`DocBlock`](ast::DocBlock) into a [`NodeDoc`].
fn node_doc(doc: &ast::DocBlock) -> NodeDoc {
    let joined = |lines: &[String]| (!lines.is_empty()).then(|| lines.join("\n"));
    NodeDoc {
        summary: joined(&doc.summary),
        extended: joined(&doc.extended),
        tags: doc.tags.iter().map(|t| t.text.clone()).collect(),
    }
}

/// What relationship an [`Edge`] expresses (`LANG.md` §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// A child node's `for` parent link (child → parent).
    ForParent,
    /// A body call from the owning node to the resolved target (label = method).
    Call,
    /// A synthesised trigger initiator → the triggered callable.
    Trigger,
    /// A `from` composition source → the composed type.
    Provenance,
}

/// A typed, directed relationship between two graph node FQNs.
///
/// `to` (and for some kinds `from`) may name a node not present as a
/// [`GraphNode`] — a synthesised trigger initiator (`event:<FQN>`, `scheduler`,
/// …), or a target that does not resolve. The emit crate decides how to render
/// such endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    /// Source endpoint FQN.
    pub from: String,
    /// Target endpoint FQN.
    pub to: String,
    /// The relationship kind.
    pub kind: EdgeKind,
    /// Edge label: the method name for `Call`, otherwise empty.
    pub label: String,
    /// Source range that produced the edge — the call site for a `Call`, so an
    /// architectural lint can point its diagnostic at the offending call. The
    /// empty span (`0..0`) for a synthesised edge (trigger, provenance, `for`).
    pub span: Span,
}

/// One step in a callable's ordered sequence trace (`LANG.md` §7, §9.2).
///
/// Calls in a chained expression are emitted left-to-right as separate steps;
/// control flow becomes `Alt` / `Loop` frames whose bodies are nested step
/// lists, in §7 evaluation order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Step {
    /// A call to another node: `Target.method(..)`.
    Call {
        /// The resolved target node FQN (as written if it does not resolve).
        target_fqn: String,
        /// The invoked method name.
        method: String,
    },
    /// A `self.method(..)` call — a self-message (ADR-004).
    SelfCall {
        /// The invoked method name.
        method: String,
    },
    /// A `return [Expr]` — a return message. `marker` is the built-in
    /// constructor (`"Ok"` / `"Err"` / `"Some"` / `"None"`) when the returned
    /// expression is one, else empty.
    Return {
        /// The marker (`Ok`/`Err`/`Some`/`None`) if any, else empty.
        marker: String,
    },
    /// An `if`/`else` → `alt` frame.
    Alt {
        /// The condition label (source text of the guard).
        cond_label: String,
        /// The then-arm body, in order.
        then: Vec<Step>,
        /// The else-arm body (empty when there is no `else`).
        r#else: Vec<Step>,
    },
    /// A `for`/`while` → `loop` frame.
    Loop {
        /// The loop's condition/iterator label.
        cond_label: String,
        /// The loop body, in order.
        body: Vec<Step>,
    },
}

/// A `feature` BDD scenario attached to its target node (`LANG.md` §5.2, §9.3).
///
/// Steps are prose, not resolved against the model, so a scenario adds no edges;
/// it renders as a given/when/then card on its target node's doc page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    /// The scenario's name.
    pub name: String,
    /// The target node's FQN (canonicalised; as written if it does not resolve).
    pub target_fqn: String,
    /// The FQN of the module that declares the feature.
    pub module: String,
    /// Source span of the feature's name (a host's go-to-definition target).
    pub span: Span,
    /// Documentation lifted from the feature's `///` block.
    pub doc: NodeDoc,
    /// The ordered given/when/then steps.
    pub steps: Vec<ScenarioStep>,
}

/// One step of a [`Scenario`] (`LANG.md` §5.2): a keyword and its prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioStep {
    /// The step keyword (`given`/`when`/`then`/`and`/`but`).
    pub keyword: String,
    /// The step's prose, with the string literal's surrounding quotes stripped.
    pub text: String,
}

/// The resolved relationship graph for a whole workspace (`LANG.md` §9).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    /// Every node, in source-declaration order across modules.
    nodes: Vec<GraphNode>,
    /// Every typed relationship.
    edges: Vec<Edge>,
    /// Sequence traces, keyed by the owning callable's FQN.
    bodies: FxHashMap<String, Vec<Step>>,
    /// Node index by FQN (positions into `nodes`).
    index: FxHashMap<String, usize>,
    /// Feature scenarios, in source-declaration order across modules (§5.2).
    scenarios: Vec<Scenario>,
}

impl Graph {
    /// Builds the graph as a pure projection of a resolved [`Workspace`].
    #[must_use]
    pub fn build(workspace: &Workspace) -> Self {
        let mut builder = Builder {
            graph: Graph::default(),
            workspace,
        };
        for entry in workspace.modules() {
            builder.collect_module(entry);
        }
        builder.graph
    }

    /// Looks up a node by its FQN.
    #[must_use]
    pub fn node(&self, fqn: &str) -> Option<&GraphNode> {
        self.index.get(fqn).map(|&i| &self.nodes[i])
    }

    /// Every node, in source-declaration order.
    #[must_use]
    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    /// Every edge, in collection order.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Iterates the nodes whose `parent` is `fqn` (its direct children).
    pub fn children_of<'a>(&'a self, fqn: &'a str) -> impl Iterator<Item = &'a GraphNode> {
        self.nodes
            .iter()
            .filter(move |n| n.parent.as_deref() == Some(fqn))
    }

    /// Iterates the nodes of a given kind.
    pub fn nodes_of_kind(&self, kind: NodeKind) -> impl Iterator<Item = &GraphNode> {
        self.nodes.iter().filter(move |n| n.kind == kind)
    }

    /// Iterates the edges of a given kind.
    pub fn edges_of_kind(&self, kind: EdgeKind) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |e| e.kind == kind)
    }

    /// The ordered sequence trace for the callable named `fqn`, if it is a
    /// disclosed callable.
    #[must_use]
    pub fn body(&self, fqn: &str) -> Option<&[Step]> {
        self.bodies.get(fqn).map(Vec::as_slice)
    }

    /// Every feature scenario, in source-declaration order (§5.2).
    #[must_use]
    pub fn scenarios(&self) -> &[Scenario] {
        &self.scenarios
    }

    /// The scenarios whose `for` target is `fqn` — the features documenting that
    /// node (§9.3).
    pub fn scenarios_of<'a>(&'a self, fqn: &'a str) -> impl Iterator<Item = &'a Scenario> {
        self.scenarios.iter().filter(move |s| s.target_fqn == fqn)
    }
}

struct Builder<'a> {
    graph: Graph,
    workspace: &'a Workspace,
}

impl Builder<'_> {
    fn collect_module(&mut self, entry: &ModuleEntry) {
        for item in &entry.ast.items {
            match item {
                Item::Decl(decl) => self.collect_decl(decl, &entry.fqn, None, entry),
                Item::Feature(feature) => self.collect_feature(feature, &entry.fqn),
            }
        }
    }

    /// Records a feature scenario against its (canonicalised) target node FQN
    /// (§5.2, §9.3).
    fn collect_feature(&mut self, feature: &Feature, module: &str) {
        let target_fqn = self.canonicalize(&path_str(&feature.target), module);
        let steps = feature
            .steps
            .iter()
            .map(|step| ScenarioStep {
                keyword: step.kind.keyword().to_owned(),
                text: literal_text(&step.text),
            })
            .collect();
        self.graph.scenarios.push(Scenario {
            name: feature.name.name.clone(),
            target_fqn,
            module: module.to_owned(),
            span: feature.name.span,
            doc: node_doc(&feature.doc),
            steps,
        });
    }

    /// Adds the node(s) for one declaration. `enclosing` is the FQN of the
    /// node this declaration is nested inside (the structural parent for a
    /// nested decl), `None` at module top level. A `container`/`component`'s
    /// recorded parent is its `for` target, not the lexical enclosure.
    fn collect_decl(
        &mut self,
        decl: &Decl,
        module: &str,
        enclosing: Option<&str>,
        entry: &ModuleEntry,
    ) {
        match &decl.kind {
            DeclKind::Person(node) | DeclKind::System(node) => {
                let fqn = self.add_node(node, decl, module, None);
                self.collect_node_body(node, &fqn, module, entry);
            }
            DeclKind::Container(node) | DeclKind::Component(node) => {
                let parent = node
                    .parent
                    .as_ref()
                    .map(|p| self.canonicalize(&path_str(p), module));
                let fqn = self.add_node(node, decl, module, parent);
                self.collect_node_body(node, &fqn, module, entry);
            }
            DeclKind::Data(data) => {
                let fqn = qualify(module, &data.name.name);
                self.push_node(GraphNode {
                    fqn: fqn.clone(),
                    name: data.name.name.clone(),
                    kind: NodeKind::Data,
                    parent: enclosing.map(str::to_owned),
                    module: module.to_owned(),
                    visibility: Visibility::from_bool(decl.is_public),
                    span: data.name.span,
                    triggers: Vec::new(),
                    signature: None,
                    shape: Some(data_shape(&data.body)),
                    doc: node_doc(&decl.doc),
                });
                if let ast::DataBody::Union(variants) = &data.body {
                    for variant in variants {
                        if let Some(record) = &variant.record {
                            let vfqn = qualify(module, &variant.name.name);
                            self.push_node(GraphNode {
                                fqn: vfqn,
                                name: variant.name.name.clone(),
                                kind: NodeKind::Data,
                                parent: enclosing.map(str::to_owned),
                                module: module.to_owned(),
                                visibility: Visibility::from_bool(decl.is_public),
                                span: variant.name.span,
                                triggers: Vec::new(),
                                signature: None,
                                shape: Some(DataShape::Record {
                                    fields: record_fields(record),
                                }),
                                // A union variant carries no `///` block of its
                                // own; its docs live on the parent `data`.
                                doc: NodeDoc::default(),
                            });
                        }
                    }
                }
            }
            // A constant is not a C4 construct (ADR-039) — no node, no edge.
            DeclKind::Constant(_) => {}
        }
    }

    fn add_node(
        &mut self,
        node: &Node,
        decl: &Decl,
        module: &str,
        parent: Option<String>,
    ) -> String {
        let fqn = qualify(module, &node.name.name);
        self.push_node(GraphNode {
            fqn: fqn.clone(),
            name: node.name.name.clone(),
            kind: NodeKind::from_ast(node.kind),
            parent,
            module: module.to_owned(),
            visibility: Visibility::from_bool(decl.is_public),
            span: node.name.span,
            triggers: Vec::new(),
            signature: None,
            shape: None,
            doc: node_doc(&decl.doc),
        });
        fqn
    }

    fn collect_node_body(
        &mut self,
        node: &Node,
        owner_fqn: &str,
        module: &str,
        entry: &ModuleEntry,
    ) {
        let Some(members) = &node.body else { return };
        for member in members {
            match member {
                BodyMember::Callable(callable) => {
                    self.collect_callable(callable, owner_fqn, module, entry);
                }
                BodyMember::Decl(decl) => {
                    self.collect_decl(decl, module, Some(owner_fqn), entry);
                }
            }
        }
    }

    fn collect_callable(
        &mut self,
        callable: &Callable,
        owner_fqn: &str,
        module: &str,
        entry: &ModuleEntry,
    ) {
        let fqn = format!("{owner_fqn}::{}", callable.name.name);
        let triggers: Vec<Trigger> = callable.macros.iter().filter_map(trigger_of).collect();
        self.push_node(GraphNode {
            fqn: fqn.clone(),
            name: callable.name.name.clone(),
            kind: NodeKind::Callable,
            parent: Some(owner_fqn.to_owned()),
            module: module.to_owned(),
            visibility: Visibility::from_bool(callable.is_public),
            span: callable.name.span,
            triggers: triggers.clone(),
            signature: Some(signature_of(callable)),
            shape: None,
            doc: node_doc(&callable.doc),
        });

        // §9.1: each trigger adds an inbound edge from its synthesised initiator.
        for trigger in &triggers {
            self.graph.edges.push(Edge {
                from: trigger.initiator(),
                to: fqn.clone(),
                kind: EdgeKind::Trigger,
                span: Span::new(0, 0),
                label: String::new(),
            });
        }

        if let Some(body) = &callable.body {
            let steps = self.trace_block(body, owner_fqn, module, entry);
            self.graph.bodies.insert(fqn, steps);
        }
    }

    // --- body tracing (§7 / §9.2) ---------------------------------------------

    fn trace_block(
        &mut self,
        block: &Block,
        owner_fqn: &str,
        module: &str,
        entry: &ModuleEntry,
    ) -> Vec<Step> {
        let mut steps = Vec::new();
        for stmt in &block.stmts {
            self.trace_stmt(stmt, owner_fqn, module, entry, &mut steps);
        }
        steps
    }

    fn trace_stmt(
        &mut self,
        stmt: &Stmt,
        owner_fqn: &str,
        module: &str,
        entry: &ModuleEntry,
        out: &mut Vec<Step>,
    ) {
        match &stmt.kind {
            StmtKind::Assign { value, .. } | StmtKind::Expr(value) => {
                self.trace_expr(value, owner_fqn, module, entry, out);
            }
            StmtKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.trace_expr(expr, owner_fqn, module, entry, out);
                }
                out.push(Step::Return {
                    marker: return_marker(expr.as_ref()),
                });
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.trace_expr(cond, owner_fqn, module, entry, out);
                let then = self.trace_block(then_block, owner_fqn, module, entry);
                let r#else = else_block
                    .as_ref()
                    .map(|b| self.trace_block(b, owner_fqn, module, entry))
                    .unwrap_or_default();
                out.push(Step::Alt {
                    cond_label: expr_label(cond),
                    then,
                    r#else,
                });
            }
            StmtKind::For { iter, body, .. } => {
                self.trace_expr(iter, owner_fqn, module, entry, out);
                let body = self.trace_block(body, owner_fqn, module, entry);
                out.push(Step::Loop {
                    cond_label: expr_label(iter),
                    body,
                });
            }
            StmtKind::While { cond, body } => {
                self.trace_expr(cond, owner_fqn, module, entry, out);
                let body = self.trace_block(body, owner_fqn, module, entry);
                out.push(Step::Loop {
                    cond_label: expr_label(cond),
                    body,
                });
            }
        }
    }

    /// Walks an expression, emitting one step per call left-to-right (§9.2) and
    /// a `Call`/`Trigger`-flavoured graph edge for each cross-boundary call.
    fn trace_expr(
        &mut self,
        expr: &Expr,
        owner_fqn: &str,
        module: &str,
        entry: &ModuleEntry,
        out: &mut Vec<Step>,
    ) {
        match &expr.kind {
            ExprKind::Postfix { base, segments } => {
                // Recurse into the base first (left-to-right): a base that is
                // itself a chain (`a.b().c()`) emits its calls before this one.
                let base_target = self.trace_base(base, owner_fqn, module, entry, out);
                for seg in segments {
                    if let Some(args) = &seg.call_args {
                        for arg in args {
                            self.trace_expr(arg, owner_fqn, module, entry, out);
                        }
                        self.emit_call(base.as_ref(), &base_target, seg, owner_fqn, out);
                    }
                }
            }
            ExprKind::From { ty, source } => {
                let sources = source.sources();
                for src in sources {
                    self.trace_expr(src, owner_fqn, module, entry, out);
                }
                // §7.2 / §9.1: each source derives the composed type.
                let composed = self.canonicalize(&resolve_path(&ty.name, module), module);
                for src in sources {
                    if let Some(source_fqn) = expr_node_fqn(src, owner_fqn, module) {
                        self.graph.edges.push(Edge {
                            from: self.canonicalize(&source_fqn, module),
                            to: composed.clone(),
                            kind: EdgeKind::Provenance,
                            label: String::new(),
                            span: Span::new(0, 0),
                        });
                    }
                }
            }
            ExprKind::Marker { payload, .. } => {
                if let Some(payload) = payload {
                    self.trace_expr(payload, owner_fqn, module, entry, out);
                }
            }
            ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) => {
                self.trace_expr(expr, owner_fqn, module, entry, out);
            }
            ExprKind::Binary { left, right, .. } => {
                self.trace_expr(left, owner_fqn, module, entry, out);
                self.trace_expr(right, owner_fqn, module, entry, out);
            }
            ExprKind::Ref(_) | ExprKind::Literal(_) => {}
        }
    }

    /// Resolves and (recursively) traces the base of a postfix chain, returning
    /// the FQN the leading call target resolves to (the chain's anchor).
    fn trace_base(
        &mut self,
        base: &Expr,
        owner_fqn: &str,
        module: &str,
        entry: &ModuleEntry,
        out: &mut Vec<Step>,
    ) -> CallTarget {
        match &base.kind {
            ExprKind::Ref(Ref::SelfNode(_)) => CallTarget::SelfNode,
            ExprKind::Ref(Ref::Path(path)) => {
                CallTarget::Node(self.canonicalize(&path_str(path), module))
            }
            _ => {
                self.trace_expr(base, owner_fqn, module, entry, out);
                CallTarget::Local
            }
        }
    }

    /// Emits one call step (and, if cross-boundary, a `Call` edge) for a single
    /// `.method(args)` segment over a resolved base.
    fn emit_call(
        &mut self,
        base: &Expr,
        base_target: &CallTarget,
        seg: &PostfixSeg,
        owner_fqn: &str,
        out: &mut Vec<Step>,
    ) {
        let method = seg.name.name.clone();
        match base_target {
            CallTarget::SelfNode if is_self(base) => {
                out.push(Step::SelfCall { method });
            }
            CallTarget::Node(target_fqn) => {
                out.push(Step::Call {
                    target_fqn: target_fqn.clone(),
                    method: method.clone(),
                });
                // §9.1: a cross-boundary call (target node differs from the
                // owner) is a `Call` edge owner → target.
                if target_fqn != owner_fqn {
                    self.graph.edges.push(Edge {
                        from: owner_fqn.to_owned(),
                        to: target_fqn.clone(),
                        kind: EdgeKind::Call,
                        label: method,
                        span: seg.span,
                    });
                }
            }
            // A method on a local value / intermediate result (`x.f()`,
            // `a.b().c()`): a self-message on the owner — local to the body.
            _ => out.push(Step::SelfCall { method }),
        }
    }

    fn push_node(&mut self, node: GraphNode) {
        // §9.1: a node with a parent contributes a `for`-parent edge child →
        // parent. Callables and nested decls carry a parent too, but only
        // container/component `for` links are structural C4 edges; a callable's
        // owner and a data's lexical enclosure are recorded on the node, not as
        // a `ForParent` edge.
        if matches!(node.kind, NodeKind::Container | NodeKind::Component)
            && let Some(parent) = &node.parent
        {
            self.graph.edges.push(Edge {
                from: node.fqn.clone(),
                to: parent.clone(),
                kind: EdgeKind::ForParent,
                label: String::new(),
                span: Span::new(0, 0),
            });
        }
        // Last declaration wins on FQN collision, matching the symbol table.
        if let Some(&i) = self.graph.index.get(&node.fqn) {
            self.graph.nodes[i] = node;
        } else {
            self.graph
                .index
                .insert(node.fqn.clone(), self.graph.nodes.len());
            self.graph.nodes.push(node);
        }
    }

    /// Canonicalises a node reference written in `module` to a full FQN.
    ///
    /// A reference is its flat FQN `module::Name` (§8.1, ADR-030). Resolution
    /// tries the path as written (an alias-expanded or cross-module FQN already
    /// resolves), then `module::<path>` for a bare local name. A structural
    /// drill (`Syntax::Parser` — container `Syntax`, component `Parser`) is not
    /// an FQN: it does not resolve and is returned as written, so the checker
    /// rejects it (§8.1) rather than the model silently building a phantom edge.
    fn canonicalize(&self, reference: &str, module: &str) -> String {
        if self.workspace.symbol(reference).is_some() {
            return reference.to_owned();
        }
        let prefixed = qualify(module, reference);
        if self.workspace.symbol(&prefixed).is_some() {
            return prefixed;
        }
        reference.to_owned()
    }
}

/// How a postfix chain's base resolves for call-edge purposes.
enum CallTarget {
    /// `self` — calls are self-messages on the owner.
    SelfNode,
    /// A named node, resolved to this FQN.
    Node(String),
    /// A local value or an intermediate result of an earlier call.
    Local,
}

/// Resolves a bare-or-qualified path to an FQN in `module`.
fn resolve_path(path: &Path, module: &str) -> String {
    if path.is_simple() {
        qualify(module, &path.segments[0].name)
    } else {
        path_str(path)
    }
}

/// The node FQN a `from`-source expression denotes, if it names a node — a
/// call whose leading target is a node (`Web.lookup()` → `Web`), or `self`. A
/// bare binding reference (`a`) does not name a node, so it yields `None`: only
/// a call's *receiver* is a node, a lone identifier is a local binding.
fn expr_node_fqn(expr: &Expr, owner_fqn: &str, _module: &str) -> Option<String> {
    let ExprKind::Postfix { base, segments } = &expr.kind else {
        return None;
    };
    // A node-denoting source is a *call* on a node receiver; a plain field
    // access on a binding (`a.value`) is local, not a node.
    if !segments.iter().any(|s| s.call_args.is_some()) {
        return None;
    }
    match &base.kind {
        ExprKind::Ref(Ref::Path(path)) => Some(path_str(path)),
        ExprKind::Ref(Ref::SelfNode(_)) => Some(owner_fqn.to_owned()),
        _ => None,
    }
}

/// The [`Signature`] of a callable: its parameters and rendered return type
/// (`void` when absent). Drives the type detail on sequence-diagram messages.
fn signature_of(callable: &Callable) -> Signature {
    Signature {
        params: callable
            .params
            .iter()
            .map(|p| SigParam {
                name: p.name.name.clone(),
                ty: render_type(&p.ty),
            })
            .collect(),
        ret: callable
            .return_ty
            .as_ref()
            .map_or_else(|| "void".to_owned(), render_type),
    }
}

/// Lifts a `data` declaration's [`ast::DataBody`] into its disclosed
/// [`DataShape`] (§3.5), rendering each field type to source form.
fn data_shape(body: &ast::DataBody) -> DataShape {
    match body {
        ast::DataBody::Record(fields) => DataShape::Record {
            fields: record_fields(fields),
        },
        ast::DataBody::Union(variants) => DataShape::Union {
            variants: variants
                .iter()
                .map(|v| DataVariant {
                    name: v.name.name.clone(),
                    fields: v.record.as_deref().map(record_fields),
                })
                .collect(),
        },
        ast::DataBody::BlackBox => DataShape::BlackBox,
    }
}

/// Renders a record's fields into [`DataField`]s, in declaration order.
fn record_fields(fields: &[ast::Field]) -> Vec<DataField> {
    fields
        .iter()
        .map(|f| DataField {
            name: f.name.name.clone(),
            ty: render_type(&f.ty),
        })
        .collect()
}

/// Renders a type to its source form: `Result<Order, Rejected>`, `Account[]`.
fn render_type(ty: &ast::Type) -> String {
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

/// The `Trigger` a macro denotes, or `None` if it is not a trigger macro.
fn trigger_of(mac: &Macro) -> Option<Trigger> {
    match mac.name.segments.last()?.name.as_str() {
        "onevent" => Some(Trigger::OnEvent {
            event_fqn: onevent_event(mac).unwrap_or_default(),
        }),
        "schedule" => Some(Trigger::Schedule),
        "http" => Some(Trigger::Http),
        "manual" => Some(Trigger::Manual),
        _ => None,
    }
}

/// The event-type FQN inside `#[onevent(Event)]`.
fn onevent_event(mac: &Macro) -> Option<String> {
    match &mac.args {
        MacroArgs::List(args) => args.iter().find_map(|arg| match arg {
            MacroArg::Path(path) => Some(path_str(path)),
            _ => None,
        }),
        _ => None,
    }
}

/// A step literal's text: a string literal with its surrounding quotes
/// stripped, or the raw form of a number/bool (steps are strings in practice).
fn literal_text(lit: &Literal) -> String {
    match lit {
        Literal::String { raw, .. } => raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(raw)
            .to_owned(),
        Literal::Number { raw, .. } => raw.clone(),
        Literal::Bool { value, .. } => value.to_string(),
    }
}

/// `module::name`, or just `name` for an anonymous module.
fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}::{name}")
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

/// Whether an expression is exactly `self`.
fn is_self(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::Ref(Ref::SelfNode(_)))
}

/// The `Ok`/`Err`/`Some`/`None` marker of a returned expression, or empty if it
/// is none of them.
fn return_marker(expr: Option<&Expr>) -> String {
    match expr.map(|e| &e.kind) {
        Some(ExprKind::Marker { kind, .. }) => kind.keyword().to_owned(),
        _ => String::new(),
    }
}

/// A short human label for a condition/iterator expression — the source-shaped
/// text used as a frame label. Covers the common guard forms; anything else
/// renders generically.
fn expr_label(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Postfix { base, segments } => {
            let mut label = expr_label(base);
            for seg in segments {
                label.push('.');
                label.push_str(&seg.name.name);
                if seg.call_args.is_some() {
                    label.push_str("()");
                }
            }
            label
        }
        ExprKind::Ref(Ref::SelfNode(_)) => "self".to_owned(),
        ExprKind::Ref(Ref::Path(path)) => path_str(path),
        ExprKind::Unary { op, expr, .. } => format!("{}{}", op.spelling(), expr_label(expr)),
        // §7.5: a binary condition renders through the same labeller, so an
        // `alt`/`loop` frame shows e.g. `x > module::LIMIT`.
        ExprKind::Binary {
            left, op, right, ..
        } => format!(
            "{} {} {}",
            expr_label(left),
            op.spelling(),
            expr_label(right)
        ),
        ExprKind::Paren(expr) => format!("({})", expr_label(expr)),
        ExprKind::Literal(lit) => literal_label(lit),
        ExprKind::Marker { kind, .. } => kind.keyword().to_owned(),
        ExprKind::From { ty, .. } => format!("{} from", path_str(&ty.name)),
    }
}

/// The source-shaped text of a literal for a frame label (§9.2).
fn literal_label(lit: &Literal) -> String {
    match lit {
        Literal::Number { raw, .. } | Literal::String { raw, .. } => raw.clone(),
        Literal::Bool { value, .. } => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{WorkspaceModule, graph};

    /// A `feature` is collected as a scenario on its (canonicalised) target node,
    /// with each step's keyword and quote-stripped prose in source order (§5.2).
    #[test]
    fn feature_collected_as_scenario_on_target() {
        let src = "//! shop\n\npublic system Shop;\n\n\
feature Checkout for Shop {\n  \
given \"a cart\"\n  and \"a logged-in buyer\"\n  \
when \"checkout runs\"\n  then \"a receipt is issued\"\n}\n";
        let g = graph(&[WorkspaceModule::new("shop", src)]);

        let scenarios: Vec<_> = g.scenarios_of("shop::Shop").collect();
        assert_eq!(scenarios.len(), 1, "one scenario on the bare-named target");
        let scenario = scenarios[0];
        assert_eq!(scenario.name, "Checkout");
        assert_eq!(scenario.target_fqn, "shop::Shop");

        let keywords: Vec<&str> = scenario.steps.iter().map(|s| s.keyword.as_str()).collect();
        assert_eq!(keywords, ["given", "and", "when", "then"]);
        assert_eq!(scenario.steps[0].text, "a cart");
        assert_eq!(scenario.steps[3].text, "a receipt is issued");
    }
}
