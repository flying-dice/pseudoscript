//! View projections: laying out a [`crate::Scene`] from the resolved
//! [`Graph`] (`LANG.md` §9.1, §9.2).
//!
//! [`project`] is a [`View`]-keyed strategy over the graph. C4 views collect
//! placed nodes and routed edges; the sequence view walks a callable's
//! [`Step`] trace into lifelines, messages, and frames. A view whose target FQN
//! does not resolve to the required node kind returns an [`EmitError`] rather
//! than panicking.

use pseudoscript_model::{EdgeKind, Graph, GraphNode, NodeKind, Signature, Step, Trigger};

use crate::scene::{
    C4EdgeKind, C4Scene, C4View, Frame, FrameKind, Lifeline, Message, MessageKind, PlacedNode,
    Rect, Scene, SeqItem, SequenceScene,
};

/// Which view to project from the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// The context view: persons and systems (`LANG.md` §9.1).
    Context,
    /// One system's containers.
    Container {
        /// The system FQN to scope to.
        of: String,
    },
    /// One container's components.
    Component {
        /// The container FQN to scope to.
        of: String,
    },
    /// The sequence trace of a triggered callable (`LANG.md` §9.2).
    Sequence {
        /// The entry callable FQN.
        entry: String,
    },
}

/// Why a projection could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    /// The view's target FQN names no node in the graph.
    UnknownNode(String),
    /// The target node exists but is the wrong kind for the requested view.
    WrongKind {
        /// The target FQN.
        fqn: String,
        /// The kind the view requires.
        expected: NodeKind,
        /// The kind the node actually is.
        found: NodeKind,
    },
    /// The sequence entry is not a callable with a disclosed body.
    NoBody(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::UnknownNode(fqn) => write!(f, "no node named {fqn}"),
            EmitError::WrongKind {
                fqn,
                expected,
                found,
            } => write!(
                f,
                "node {fqn} is a {}, expected a {}",
                found.keyword(),
                expected.keyword()
            ),
            EmitError::NoBody(fqn) => write!(f, "callable {fqn} has no disclosed body"),
        }
    }
}

impl std::error::Error for EmitError {}

/// Projects `view` from `graph` into a laid-out [`Scene`].
///
/// # Errors
///
/// Returns [`EmitError`] when the view's target FQN does not resolve to the
/// required node kind, or a sequence entry has no disclosed body.
pub fn project(graph: &Graph, view: View) -> Result<Scene, EmitError> {
    project_view(graph, view)
}

/// Projects the diagram that best explains a single symbol, picking the view
/// from the symbol's kind: a callable with a disclosed body traces as a
/// sequence; a system shows its containers, a container or component its
/// components. A bodyless callable falls back to its owner's structural view; a
/// person or `data` symbol has no boundary, so the context overview stands in.
///
/// This is the compiler's "what diagram fits this symbol" decision, so a host
/// (an editor hover, a side panel) does not encode view-selection logic itself.
///
/// # Errors
///
/// Returns [`EmitError::UnknownNode`] when `fqn` names no graph node.
pub fn project_symbol(graph: &Graph, fqn: &str) -> Result<Scene, EmitError> {
    let node = graph
        .node(fqn)
        .ok_or_else(|| EmitError::UnknownNode(fqn.to_owned()))?;
    if node.kind == NodeKind::Callable {
        return match project_view(
            graph,
            View::Sequence {
                entry: fqn.to_owned(),
            },
        ) {
            // A declared-but-bodyless callable cannot trace; show where it lives.
            Err(EmitError::NoBody(_)) => {
                let owner = node
                    .parent
                    .clone()
                    .ok_or_else(|| EmitError::UnknownNode(fqn.to_owned()))?;
                structural_view(graph, &owner)
            }
            other => other,
        };
    }
    structural_view(graph, fqn)
}

/// The structural boundary view for a node: a system's containers, a
/// container's components, a component's sibling components (its parent
/// container's view). Persons and `data` fall back to the context overview.
fn structural_view(graph: &Graph, fqn: &str) -> Result<Scene, EmitError> {
    let node = graph
        .node(fqn)
        .ok_or_else(|| EmitError::UnknownNode(fqn.to_owned()))?;
    let view = match node.kind {
        NodeKind::System => View::Container { of: fqn.to_owned() },
        NodeKind::Container => View::Component { of: fqn.to_owned() },
        NodeKind::Component => View::Component {
            of: node.parent.clone().unwrap_or_else(|| fqn.to_owned()),
        },
        NodeKind::Person | NodeKind::Data | NodeKind::Callable => View::Context,
    };
    project_view(graph, view)
}

fn project_view(graph: &Graph, view: View) -> Result<Scene, EmitError> {
    match view {
        View::Context => Ok(Scene::C4(project_context(graph))),
        View::Container { of } => Ok(Scene::C4(project_boundary(
            graph,
            &of,
            NodeKind::System,
            NodeKind::Container,
            C4View::Container,
        )?)),
        View::Component { of } => Ok(Scene::C4(project_boundary(
            graph,
            &of,
            NodeKind::Container,
            NodeKind::Component,
            C4View::Component,
        )?)),
        View::Sequence { entry } => Ok(Scene::Sequence(project_sequence(graph, &entry)?)),
    }
}

// --- C4 projections ---------------------------------------------------------

/// The context view: every person and system, with inter-system edges, trigger
/// edges into systems, and provenance (`LANG.md` §9.1). Edges bubble to the
/// enclosing system, so a body call between containers of two systems is a
/// single system → system edge.
fn project_context(graph: &Graph) -> C4Scene {
    let nodes: Vec<PlacedNode> = graph
        .nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Person | NodeKind::System))
        .map(|n| placed(n, None))
        .collect();
    let in_view: Vec<&str> = nodes.iter().map(|n| n.fqn.as_str()).collect();
    let edges = collect_edges(graph, &in_view, |fqn| system_of(graph, fqn));
    laid_out_c4(C4View::Context, None, nodes, edges)
}

/// A boundary view (container or component): the boundary node itself plus its
/// `for`-children, with edges among the children (`LANG.md` §9.1). `boundary`
/// is the kind the `of` node must be; `child` is the kind of the contained
/// nodes.
fn project_boundary(
    graph: &Graph,
    of: &str,
    boundary: NodeKind,
    child: NodeKind,
    view: C4View,
) -> Result<C4Scene, EmitError> {
    let anchor = require_kind(graph, of, boundary)?;

    let mut nodes = vec![placed(anchor, None)];
    nodes.extend(
        graph
            .children_of(of)
            .filter(|n| n.kind == child)
            .map(|n| placed(n, Some(of.to_owned()))),
    );

    // External actors that interact with the boundary's nodes — the persons and
    // other systems (and, for a component view, other containers) that call in
    // or are called out to. Drawn outside the frame (`boundary: None`), as a C4
    // boundary diagram does (`LANG.md` §9.1).
    let inside: Vec<String> = nodes.iter().map(|n| n.fqn.clone()).collect();
    let inside_refs: Vec<&str> = inside.iter().map(String::as_str).collect();
    nodes.extend(external_actors(graph, of, child, &inside_refs));

    // Edges among nodes in view, lifting each endpoint to the contained child it
    // belongs to (a call from a component bubbles to its owning container, etc.).
    let in_view: Vec<&str> = nodes.iter().map(|n| n.fqn.as_str()).collect();
    let edges = collect_edges(graph, &in_view, |fqn| lift_to_view(graph, fqn, &in_view));

    Ok(laid_out_c4(view, Some(of.to_owned()), nodes, edges))
}

/// The external actors a boundary view draws around its frame: persons and
/// systems (and, in a component view, other containers) outside the boundary
/// that have a call/trigger/provenance edge to or from a node inside it.
/// Sorted by FQN for determinism.
fn external_actors(graph: &Graph, of: &str, child: NodeKind, inside: &[&str]) -> Vec<PlacedNode> {
    let mut fqns: Vec<String> = Vec::new();
    for edge in graph.edges() {
        if c4_edge_kind(edge.kind).is_none() {
            continue;
        }
        let from_in = lift_to_view(graph, &edge.from, inside).is_some();
        let to_in = lift_to_view(graph, &edge.to, inside).is_some();
        let outside = match (from_in, to_in) {
            (true, false) => &edge.to,
            (false, true) => &edge.from,
            _ => continue, // wholly inside or wholly outside the boundary
        };
        if let Some(actor) = presentable_external(graph, outside, of, child)
            && !fqns.contains(&actor)
        {
            fqns.push(actor);
        }
    }
    fqns.sort();
    fqns.iter()
        .filter_map(|f| graph.node(f).map(|n| placed(n, None)))
        .collect()
}

/// The node to draw for an external endpoint: walk up to the first ancestor
/// that reads as a peer actor at this view's level — a person or system always,
/// and a container too in a component view. `None` for a synthesised initiator
/// (no node) or the boundary itself.
fn presentable_external(graph: &Graph, fqn: &str, of: &str, child: NodeKind) -> Option<String> {
    let mut current = fqn.to_owned();
    loop {
        let node = graph.node(&current)?;
        let is_actor = matches!(node.kind, NodeKind::Person | NodeKind::System)
            || (node.kind == NodeKind::Container && child == NodeKind::Component);
        if is_actor && current != of {
            return Some(current);
        }
        current = node.parent.clone()?;
    }
}

/// Maps a graph endpoint FQN to the in-view node it should attach to, or `None`
/// to drop the edge. `lift` walks an endpoint up to a node present in the view;
/// a synthesised initiator (`event:…`, `scheduler`, …) has no `lift` mapping and
/// is kept as-is so trigger edges survive.
fn collect_edges(
    graph: &Graph,
    in_view: &[&str],
    lift: impl Fn(&str) -> Option<String>,
) -> Vec<RoutedEdges> {
    let mut edges: Vec<RoutedEdges> = Vec::new();
    for edge in graph.edges() {
        let Some(kind) = c4_edge_kind(edge.kind) else {
            continue;
        };
        let from = endpoint(edge.kind, &edge.from, in_view, &lift, EndpointSide::From);
        let to = endpoint(edge.kind, &edge.to, in_view, &lift, EndpointSide::To);
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        if from == to {
            continue;
        }
        edges.push(RoutedEdges {
            from,
            to,
            kind,
            label: edge.label.clone(),
        });
    }
    edges.sort_by(|a, b| {
        (&a.from, &a.to, a.kind.keyword(), &a.label).cmp(&(
            &b.from,
            &b.to,
            b.kind.keyword(),
            &b.label,
        ))
    });
    edges.dedup();
    edges
}

/// Working edge form before placement into the scene (same shape as
/// [`crate::scene::RoutedEdge`]).
type RoutedEdges = crate::scene::RoutedEdge;

/// Which side of an edge an endpoint is.
#[derive(Clone, Copy)]
enum EndpointSide {
    From,
    To,
}

/// Resolves one edge endpoint to an in-view node FQN, or `None` to drop it.
///
/// A trigger edge's `from` is a synthesised initiator (kept verbatim, no node);
/// every other endpoint must lift to a node present in the view.
fn endpoint(
    kind: EdgeKind,
    fqn: &str,
    in_view: &[&str],
    lift: &impl Fn(&str) -> Option<String>,
    side: EndpointSide,
) -> Option<String> {
    if matches!((kind, side), (EdgeKind::Trigger, EndpointSide::From)) {
        // The initiator is not a declared node; keep it verbatim.
        return Some(fqn.to_owned());
    }
    if in_view.contains(&fqn) {
        return Some(fqn.to_owned());
    }
    lift(fqn).filter(|l| in_view.iter().any(|v| v == l))
}

/// Maps the model's [`EdgeKind`] to the C4 scene edge kind, dropping the
/// structural `for`-parent edges (containment is the `in` attribute, not an
/// arrow).
fn c4_edge_kind(kind: EdgeKind) -> Option<C4EdgeKind> {
    match kind {
        EdgeKind::Call => Some(C4EdgeKind::Call),
        EdgeKind::Trigger => Some(C4EdgeKind::Trigger),
        EdgeKind::Provenance => Some(C4EdgeKind::Provenance),
        EdgeKind::ForParent => None,
    }
}

/// Lifts an endpoint to the in-view node it belongs to: the endpoint itself if
/// present, else an ancestor (`parent` chain) that is in view.
fn lift_to_view(graph: &Graph, fqn: &str, in_view: &[&str]) -> Option<String> {
    let mut current = fqn.to_owned();
    loop {
        if in_view.iter().any(|v| *v == current) {
            return Some(current);
        }
        let node = graph.node(&current)?;
        current = node.parent.clone()?;
    }
}

/// The enclosing `system` of a node, walking the `parent` chain. Used by the
/// context view to bubble cross-container calls to system → system edges.
fn system_of(graph: &Graph, fqn: &str) -> Option<String> {
    let mut current = fqn.to_owned();
    loop {
        let node = graph.node(&current)?;
        if node.kind == NodeKind::System {
            return Some(current);
        }
        current = node.parent.clone()?;
    }
}

/// Builds a placed node with a default layout rect; [`crate::render`] assigns
/// real coordinates.
fn placed(node: &GraphNode, boundary: Option<String>) -> PlacedNode {
    PlacedNode {
        fqn: node.fqn.clone(),
        kind: node.kind,
        label: node.name.clone(),
        summary: node.doc.summary.clone(),
        boundary,
        rect: Rect::default(),
    }
}

/// Assembles a C4 scene and lays it out for the renderer.
fn laid_out_c4(
    view: C4View,
    of: Option<String>,
    nodes: Vec<PlacedNode>,
    edges: Vec<RoutedEdges>,
) -> C4Scene {
    let mut scene = C4Scene {
        view,
        of,
        nodes,
        edges,
    };
    crate::render::layout_c4(&mut scene);
    scene
}

/// Looks a node up and asserts its kind, mapping failures to [`EmitError`].
fn require_kind<'a>(
    graph: &'a Graph,
    fqn: &str,
    expected: NodeKind,
) -> Result<&'a GraphNode, EmitError> {
    let node = graph
        .node(fqn)
        .ok_or_else(|| EmitError::UnknownNode(fqn.to_owned()))?;
    if node.kind == expected {
        Ok(node)
    } else {
        Err(EmitError::WrongKind {
            fqn: fqn.to_owned(),
            expected,
            found: node.kind,
        })
    }
}

// --- sequence projection ----------------------------------------------------

/// The sequence view: a triggered callable's `body` trace as lifelines,
/// messages, and frames (`LANG.md` §9.2, §7).
///
/// The initiator (the trigger's actor, or a generic `caller` for a callable
/// projected directly) is the first lifeline: it calls the entry and receives
/// the entry's return. Calls to disclosed callees expand inline — the active
/// lifeline switches to the callee, whose `return`s flow back to its caller —
/// while black-box callees and recursive calls stay single messages.
fn project_sequence(graph: &Graph, entry: &str) -> Result<SequenceScene, EmitError> {
    let node = require_kind(graph, entry, NodeKind::Callable)?;
    let body = graph
        .body(entry)
        .ok_or_else(|| EmitError::NoBody(entry.to_owned()))?;

    // The owner (the callable's parent node) executes the entry body. A real
    // trigger actor (client/scheduler/event) is a meaningful lifeline that calls
    // in and receives the entry's return. A callable with no trigger has only a
    // generic `caller`, which just restates the diagram title — omit its lifeline
    // and inbound call; the owner is the root, and the entry's returns render as
    // short left-edge stubs (still referencing `caller`, which has no lifeline).
    let owner = node.parent.clone().unwrap_or_else(|| entry.to_owned());
    let actor = node
        .triggers
        .first()
        .map_or_else(|| "caller".to_owned(), Trigger::initiator);
    // A real trigger actor (client/scheduler/event) is a meaningful lifeline that
    // calls in and receives the entry's returns. The generic `caller` just
    // restates the title: it gets no lifeline or inbound call, and the entry's
    // returns are suppressed (an empty `caller` signals this to `trace_body`) —
    // they only make sense once a real incoming trigger exists.
    let has_actor = actor != "caller";

    let mut order = Vec::new();
    let mut items = Vec::new();
    if has_actor {
        order.push(actor.clone());
        items.push(SeqItem::Message(Message {
            from: actor.clone(),
            to: owner.clone(),
            kind: MessageKind::Call,
            label: node.name.clone(),
            detail: node.signature.as_ref().map(call_detail).unwrap_or_default(),
        }));
    }
    order.push(owner.clone());
    let caller = if has_actor { actor.as_str() } else { "" };

    // The call stack guards against re-expanding a callable already in flight
    // (direct or mutual recursion).
    let mut stack = vec![entry.to_owned()];
    items.extend(trace_body(
        graph, &owner, caller, body, &mut order, &mut stack,
    ));

    // Each participant's kind drives its lifeline-head card; a synthesised
    // initiator reads as a person actor, an unresolved target defaults to a
    // container. Coordinates are assigned by the layout crate at render time.
    let participants = order
        .into_iter()
        .map(|fqn| {
            let kind = graph.node(&fqn).map_or_else(
                || {
                    if is_initiator(&fqn) {
                        NodeKind::Person
                    } else {
                        NodeKind::Container
                    }
                },
                |n| n.kind,
            );
            Lifeline { fqn, kind }
        })
        .collect();

    Ok(SequenceScene {
        entry: entry.to_owned(),
        participants,
        items,
    })
}

/// Traces the body of `active` (the executing node) into ordered items. `caller`
/// is the lifeline `active`'s `return`s land on; `stack` is the callables in
/// flight. Disclosed callees expand inline (the active lifeline switches to the
/// callee); black-box callees and callees already on the stack render as a
/// single message with no expansion (`LANG.md` §9.2).
fn trace_body(
    graph: &Graph,
    active: &str,
    caller: &str,
    steps: &[Step],
    order: &mut Vec<String>,
    stack: &mut Vec<String>,
) -> Vec<SeqItem> {
    let mut items = Vec::new();
    let mut i = 0;
    while i < steps.len() {
        match &steps[i] {
            Step::Call { target_fqn, method } => {
                register(order, target_fqn);
                let invoked = format!("{target_fqn}::{method}");
                let sig = graph.node(&invoked).and_then(|n| n.signature.as_ref());
                items.push(SeqItem::Message(Message {
                    from: active.to_owned(),
                    to: target_fqn.clone(),
                    kind: MessageKind::Call,
                    label: method.clone(),
                    detail: sig.map(call_detail).unwrap_or_default(),
                }));
                if let Some(invoked_body) = graph.body(&invoked)
                    && !stack.iter().any(|f| f == &invoked)
                {
                    // A disclosed callee expands inline; its own returns flow back.
                    stack.push(invoked.clone());
                    items.extend(trace_body(
                        graph,
                        target_fqn,
                        active,
                        invoked_body,
                        order,
                        stack,
                    ));
                    stack.pop();
                } else {
                    // A leaf (or already-in-flight) call still returns: synthesise
                    // the out-and-back response so every call has its return. The
                    // detail is the callee's return type (empty when `void`).
                    items.push(SeqItem::Message(Message {
                        from: target_fqn.clone(),
                        to: active.to_owned(),
                        kind: MessageKind::Return,
                        label: String::new(),
                        detail: sig.map(|s| return_detail(&s.ret, "")).unwrap_or_default(),
                    }));
                }
            }
            Step::SelfCall { method } => {
                items.push(SeqItem::Message(Message {
                    from: active.to_owned(),
                    to: active.to_owned(),
                    kind: MessageKind::SelfMsg,
                    label: method.clone(),
                    detail: String::new(),
                }));
            }
            Step::Return { marker } => {
                // An empty `caller` is the suppressed generic caller (a triggerless
                // entry): there is no lifeline to return to, so drop the return —
                // it reappears once a real trigger gives the entry an incoming call.
                if !caller.is_empty() {
                    // The concrete type of this return's branch comes from the
                    // currently-executing callable's signature (top of the stack).
                    let detail = stack
                        .last()
                        .and_then(|fqn| graph.node(fqn))
                        .and_then(|n| n.signature.as_ref())
                        .map(|sig| return_detail(&sig.ret, marker))
                        .unwrap_or_default();
                    items.push(SeqItem::Message(Message {
                        from: active.to_owned(),
                        to: caller.to_owned(),
                        kind: MessageKind::Return,
                        label: marker.clone(),
                        detail,
                    }));
                }
            }
            Step::Alt {
                cond_label,
                then,
                r#else,
            } => {
                // A branch whose body traces empty (e.g. its only step is a return
                // suppressed for a triggerless entry) emits no frame.
                let then_body = trace_body(graph, active, caller, then, order, stack);
                push_frame(&mut items, FrameKind::Alt, cond_label.clone(), then_body);
                if !r#else.is_empty() {
                    let body = trace_body(graph, active, caller, r#else, order, stack);
                    push_frame(
                        &mut items,
                        FrameKind::Alt,
                        format!("else {cond_label}"),
                        body,
                    );
                } else if matches!(then.last(), Some(Step::Return { .. })) && i + 1 < steps.len() {
                    // Guard clause: `if (c) { return } rest`. The then-branch
                    // always returns, so the remaining steps in this block run
                    // only when `c` is false — they are the implicit else, and
                    // belong inside the alt as its second compartment.
                    let body = trace_body(graph, active, caller, &steps[i + 1..], order, stack);
                    push_frame(
                        &mut items,
                        FrameKind::Alt,
                        format!("else {cond_label}"),
                        body,
                    );
                    break;
                }
            }
            Step::Loop { cond_label, body } => {
                let body = trace_body(graph, active, caller, body, order, stack);
                push_frame(&mut items, FrameKind::Loop, cond_label.clone(), body);
            }
        }
        i += 1;
    }
    items
}

/// Pushes a combined-fragment frame, unless its traced body is empty (a branch
/// whose only content was suppressed renders nothing rather than an empty box).
fn push_frame(items: &mut Vec<SeqItem>, kind: FrameKind, cond: String, body: Vec<SeqItem>) {
    if !body.is_empty() {
        items.push(SeqItem::Frame(Frame { kind, cond, body }));
    }
}

/// Registers `fqn` as a participant if not already present (first-appearance
/// order).
fn register(participants: &mut Vec<String>, fqn: &str) {
    if !participants.iter().any(|p| p == fqn) {
        participants.push(fqn.to_owned());
    }
}

/// Whether an endpoint token is a synthesised trigger initiator rather than a
/// declared node (`event:<FQN>`, `scheduler`, `client`, `caller`).
fn is_initiator(token: &str) -> bool {
    token.starts_with("event:") || matches!(token, "scheduler" | "client" | "caller")
}

/// A call's type detail: `(name: ty, …): Ret`, the return type omitted when
/// `void`. Shown dimmed after the method name on a call message (`LANG.md` §9.2).
fn call_detail(sig: &Signature) -> String {
    let params = sig
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, p.ty))
        .collect::<Vec<_>>()
        .join(", ");
    if sig.ret == "void" {
        format!("({params})")
    } else {
        format!("({params}): {}", sig.ret)
    }
}

/// The concrete type a `return` carries, given the executing callable's return
/// type `ret` and the return's `marker`. `Ok`/`Some` take the first generic
/// argument of `Result`/`Option`, `Err` the second; an empty marker (a bare
/// value return) is the whole type. `None` and an absent type carry nothing.
fn return_detail(ret: &str, marker: &str) -> String {
    match marker {
        "Ok" | "Some" => generic_arg(ret, 0).unwrap_or_default(),
        "Err" => generic_arg(ret, 1).unwrap_or_default(),
        // A bare value return (empty marker) carries the whole type, unless void.
        "" if ret != "void" => ret.to_owned(),
        // `None`, a bare void return, or any other marker carries nothing.
        _ => String::new(),
    }
}

/// The `n`th top-level generic argument of a rendered type (`Result<T, E>` → `0`
/// = `T`, `1` = `E`), or `None` if the type has no `<…>` or too few arguments.
/// Splits on top-level commas, respecting nested `<…>`.
fn generic_arg(ty: &str, n: usize) -> Option<String> {
    let open = ty.find('<')?;
    let inner = ty[open + 1..].strip_suffix('>')?;
    let mut depth = 0;
    let mut args = Vec::new();
    let mut start = 0;
    for (i, ch) in inner.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                args.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    args.push(inner[start..].trim());
    args.get(n).map(|s| (*s).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;
    use pseudoscript_model::{WorkspaceModule, graph};

    /// A customer (one module) calling a container of a system (another module),
    /// whose components live in a third — the canonical C4 cross-boundary shape.
    fn workspace() -> Graph {
        let customer = WorkspaceModule::new(
            "customer".to_owned(),
            "//! customer\npublic person Customer {\n  public Use(): void { shop::Web.View() }\n}"
                .to_owned(),
        );
        let shop = WorkspaceModule::new(
            "shop".to_owned(),
            "//! shop\npublic system Shop;\npublic container Web for Shop {\n  public View(): void { shop::Api.Fetch() }\n}\npublic container Api for Shop;"
                .to_owned(),
        );
        let api = WorkspaceModule::new(
            "api".to_owned(),
            "//! api\npublic component Fetch for shop::Api {\n  public Fetch(): void { warehouse::Stock.Read() }\n}"
                .to_owned(),
        );
        let warehouse = WorkspaceModule::new(
            "warehouse".to_owned(),
            "//! warehouse\npublic system Stock {\n  public Read(): void;\n}".to_owned(),
        );
        graph(&[customer, shop, api, warehouse])
    }

    fn c4(scene: Scene) -> C4Scene {
        match scene {
            Scene::C4(s) => s,
            Scene::Sequence(_) => panic!("expected a C4 scene"),
        }
    }

    #[test]
    fn container_view_includes_external_person_caller() {
        let scene = c4(project(
            &workspace(),
            View::Container {
                of: "shop::Shop".to_owned(),
            },
        )
        .expect("projects"));
        let customer = scene
            .nodes
            .iter()
            .find(|n| n.fqn == "customer::Customer")
            .expect("external person caller appears in the container view");
        assert_eq!(
            customer.boundary, None,
            "external actor sits outside the frame"
        );
        // The boundary's own children stay framed.
        assert!(
            scene
                .nodes
                .iter()
                .any(|n| n.fqn == "shop::Web" && n.boundary.as_deref() == Some("shop::Shop"))
        );
    }

    #[test]
    fn project_symbol_picks_sequence_for_a_callable_with_a_body() {
        let scene = project_symbol(&workspace(), "shop::Web::View").expect("projects");
        // A callable with a body traces as a sequence (participants, not nodes).
        assert!(
            matches!(scene, Scene::Sequence(_)),
            "expected a sequence scene"
        );
    }

    #[test]
    fn project_symbol_picks_container_view_for_a_system() {
        let scene = c4(project_symbol(&workspace(), "shop::Shop").expect("projects"));
        assert_eq!(scene.of.as_deref(), Some("shop::Shop"));
        assert!(scene.nodes.iter().any(|n| n.fqn == "shop::Web"));
    }

    #[test]
    fn project_symbol_falls_back_to_owner_view_for_a_bodyless_callable() {
        // `warehouse::Stock::Read` is declared without a body — it cannot trace,
        // so its owning system's structural (container) view stands in.
        let scene = c4(project_symbol(&workspace(), "warehouse::Stock::Read").expect("projects"));
        assert_eq!(scene.of.as_deref(), Some("warehouse::Stock"));
    }

    #[test]
    fn component_view_includes_external_container_and_system() {
        let scene = c4(project(
            &workspace(),
            View::Component {
                of: "shop::Api".to_owned(),
            },
        )
        .expect("projects"));
        let fqns: Vec<&str> = scene.nodes.iter().map(|n| n.fqn.as_str()).collect();
        // The calling container is shown as a peer, lifted no further than itself.
        assert!(fqns.contains(&"shop::Web"), "caller container: {fqns:?}");
        // The downstream system is shown directly.
        assert!(
            fqns.contains(&"warehouse::Stock"),
            "callee system: {fqns:?}"
        );
        // Neither is enclosed by the frame.
        for ext in ["shop::Web", "warehouse::Stock"] {
            let node = scene.nodes.iter().find(|n| n.fqn == ext).unwrap();
            assert_eq!(node.boundary, None);
        }
    }
}
