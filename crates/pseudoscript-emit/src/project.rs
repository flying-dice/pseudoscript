//! View projections: laying out a [`crate::Scene`] from the resolved
//! [`Graph`] (`LANG.md` §9.1, §9.2).
//!
//! [`project`] is a [`View`]-keyed strategy over the graph. C4 views collect
//! placed nodes and routed edges; the sequence view walks a callable's
//! [`Step`] trace into lifelines, messages, and frames. A view whose target FQN
//! does not resolve to the required node kind returns an [`EmitError`] rather
//! than panicking.

use pseudoscript_model::{EdgeKind, Graph, GraphNode, NodeKind, Step};

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

    // Edges among nodes in view, lifting each endpoint to the contained child it
    // belongs to (a call from a component bubbles to its owning container, etc.).
    let in_view: Vec<&str> = nodes.iter().map(|n| n.fqn.as_str()).collect();
    let edges = collect_edges(graph, &in_view, |fqn| lift_to_view(graph, fqn, &in_view));

    Ok(laid_out_c4(view, Some(of.to_owned()), nodes, edges))
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

/// The sequence view: a callable's `body` trace as lifelines, messages, and
/// frames (`LANG.md` §9.2, §7).
fn project_sequence(graph: &Graph, entry: &str) -> Result<SequenceScene, EmitError> {
    let node = require_kind(graph, entry, NodeKind::Callable)?;
    let body = graph
        .body(entry)
        .ok_or_else(|| EmitError::NoBody(entry.to_owned()))?;

    // The owner (the callable's parent node) is the first lifeline.
    let owner = node.parent.clone().unwrap_or_else(|| entry.to_owned());

    let mut order = vec![owner.clone()];
    let items = trace_items(body, &owner, &mut order);

    // x-positions are assigned by the renderer's layout pass. Each participant's
    // kind drives its lifeline-head card; an unresolved target defaults to a
    // container.
    let participants = order
        .into_iter()
        .map(|fqn| {
            let kind = graph.node(&fqn).map_or(NodeKind::Container, |n| n.kind);
            Lifeline { fqn, kind, x: 0 }
        })
        .collect();

    let mut scene = SequenceScene {
        entry: entry.to_owned(),
        participants,
        items,
    };
    crate::render::layout_sequence(&mut scene);
    Ok(scene)
}

/// Walks a step list into ordered sequence items, registering each new
/// participant on first appearance.
fn trace_items(steps: &[Step], owner: &str, participants: &mut Vec<String>) -> Vec<SeqItem> {
    let mut items = Vec::new();
    for step in steps {
        match step {
            Step::Call { target_fqn, method } => {
                register(participants, target_fqn);
                items.push(SeqItem::Message(Message {
                    from: owner.to_owned(),
                    to: target_fqn.clone(),
                    kind: MessageKind::Call,
                    label: method.clone(),
                }));
            }
            Step::SelfCall { method } => {
                items.push(SeqItem::Message(Message {
                    from: owner.to_owned(),
                    to: owner.to_owned(),
                    kind: MessageKind::SelfMsg,
                    label: method.clone(),
                }));
            }
            Step::Return { marker } => {
                items.push(SeqItem::Message(Message {
                    from: owner.to_owned(),
                    to: owner.to_owned(),
                    kind: MessageKind::Return,
                    label: marker.clone(),
                }));
            }
            Step::Alt {
                cond_label,
                then,
                r#else,
            } => {
                items.push(SeqItem::Frame(Frame {
                    kind: FrameKind::Alt,
                    cond: cond_label.clone(),
                    body: trace_items(then, owner, participants),
                }));
                if !r#else.is_empty() {
                    items.push(SeqItem::Frame(Frame {
                        kind: FrameKind::Alt,
                        cond: format!("else {cond_label}"),
                        body: trace_items(r#else, owner, participants),
                    }));
                }
            }
            Step::Loop { cond_label, body } => {
                items.push(SeqItem::Frame(Frame {
                    kind: FrameKind::Loop,
                    cond: cond_label.clone(),
                    body: trace_items(body, owner, participants),
                }));
            }
        }
    }
    items
}

/// Registers `fqn` as a participant if not already present (first-appearance
/// order).
fn register(participants: &mut Vec<String>, fqn: &str) {
    if !participants.iter().any(|p| p == fqn) {
        participants.push(fqn.to_owned());
    }
}
