//! Adapt the resolved `PseudoScript` C4 model into the universe's internal graph.
//!
//! The universe places only the **structural** nodes — systems, containers,
//! components. Persons, data, and callables are not placed; a relationship that
//! runs through them is *lifted* to the nearest enclosing structural node, so the
//! graph carries node-to-node relationships at any level (spec §4, §7). Containment
//! lives on each node (`parent`/`children`); relationships are the petgraph edges.
//! All positions are engine outputs — zero here, filled by the simulation later.

use std::collections::{HashMap, HashSet};

use petgraph::graph::{Graph, NodeIndex};
use pseudoscript_model::{
    EdgeKind, Graph as Model, NodeKind, WorkspaceModule, graph as build_model,
};

/// An index into the universe graph (petgraph's node index).
pub type NodeIx = NodeIndex<u32>;

/// The C4 abstraction level a placed node sits at. Only these enter the universe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4Level {
    System,
    Container,
    Component,
}

/// A placed node: a system, container, or component, plus its place in the
/// containment tree. Positions/sizes are engine **outputs**, zero until computed.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    /// Stable model FQN — the layout caches and seeds its RNG from this.
    pub id: String,
    pub level: C4Level,
    /// Enclosing node in the containment tree (`None` for a top-level system).
    pub parent: Option<NodeIx>,
    pub children: Vec<NodeIx>,
    /// World position (computed in placement).
    pub pos: [f32; 3],
    /// Bounding-sphere radius after sizing.
    pub radius: f32,
    /// Position relative to the parent (intermediate, computed in sizing).
    pub rel_pos: [f32; 3],
    /// Honoured as a fixed constraint by the simulation.
    pub pinned: bool,
}

impl LayoutNode {
    fn new(id: String, level: C4Level) -> Self {
        Self {
            id,
            level,
            parent: None,
            children: Vec::new(),
            pos: [0.0; 3],
            radius: 0.0,
            rel_pos: [0.0; 3],
            pinned: false,
        }
    }
}

/// The universe: structural nodes (petgraph weights) with the containment tree on
/// each node, relationship edges between them, and the top-level systems.
pub struct Universe {
    /// Nodes are [`LayoutNode`]; edges are lifted relationships (no weight).
    pub graph: Graph<LayoutNode, ()>,
    /// Top-level systems (containment roots), in model declaration order.
    pub roots: Vec<NodeIx>,
}

/// Build the universe from module sources — the same `(fqn, source)` input the IDE
/// feeds [`pseudoscript_model::graph`].
#[must_use]
pub fn build(modules: &[WorkspaceModule]) -> Universe {
    from_model(&build_model(modules))
}

/// Adapt an already-resolved model into the universe graph.
#[must_use]
pub fn from_model(model: &Model) -> Universe {
    let mut graph: Graph<LayoutNode, ()> = Graph::new();

    // 1. Each structural model node becomes a universe node; remember its index.
    let mut ix: HashMap<&str, NodeIx> = HashMap::new();
    for node in model.nodes() {
        if let Some(level) = level_of(node.kind) {
            let nx = graph.add_node(LayoutNode::new(node.fqn.clone(), level));
            ix.insert(node.fqn.as_str(), nx);
        }
    }

    // 2. Containment: hook each node to its structural parent (collected first so
    //    the graph isn't borrowed mutably while reading model parent links).
    let links: Vec<(NodeIx, Option<NodeIx>)> = model
        .nodes()
        .iter()
        .filter_map(|node| {
            let nx = *ix.get(node.fqn.as_str())?;
            Some((nx, node.parent.as_deref().and_then(|p| ix.get(p).copied())))
        })
        .collect();
    let mut roots = Vec::new();
    for (nx, parent) in links {
        graph[nx].parent = parent;
        match parent {
            Some(p) => graph[p].children.push(nx),
            None => roots.push(nx),
        }
    }

    // 3. Relationships: lift every call to the structural level and add one edge
    //    per distinct unordered pair (the macro sim wants clusters drawn together).
    let mut seen: HashSet<(NodeIx, NodeIx)> = HashSet::new();
    for edge in model.edges() {
        if edge.kind != EdgeKind::Call {
            continue;
        }
        let (Some(a), Some(b)) = (lift(model, &edge.from, &ix), lift(model, &edge.to, &ix))
        else {
            continue;
        };
        if a == b {
            continue;
        }
        let pair = if a.index() < b.index() { (a, b) } else { (b, a) };
        if seen.insert(pair) {
            graph.add_edge(a, b, ());
        }
    }

    Universe { graph, roots }
}

/// The structural level of a model kind, or `None` for the kinds the universe does
/// not place (person, data, callable).
fn level_of(kind: NodeKind) -> Option<C4Level> {
    match kind {
        NodeKind::System => Some(C4Level::System),
        NodeKind::Container => Some(C4Level::Container),
        NodeKind::Component => Some(C4Level::Component),
        NodeKind::Person | NodeKind::Data | NodeKind::Callable => None,
    }
}

/// Walk a model FQN up its `parent` chain to the nearest structural node present
/// in the universe; `None` if the chain reaches the top without one.
fn lift(model: &Model, fqn: &str, ix: &HashMap<&str, NodeIx>) -> Option<NodeIx> {
    let mut cur = fqn;
    loop {
        if let Some(&nx) = ix.get(cur) {
            return Some(nx);
        }
        cur = model.node(cur)?.parent.as_deref()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(src: &str) -> Vec<WorkspaceModule> {
        vec![WorkspaceModule::new("m", src)]
    }

    #[test]
    fn places_structural_nodes_with_containment_and_lifts_calls() {
        // One system with two containers; a call from App's body into Svc.
        let src = "\
system Sys;
container App for m::Sys {
  Do() {
    m::Svc.Handle()
  }
}
container Svc for m::Sys {
  Handle();
}
";
        let u = build(&module(src));

        // Only the three structural nodes are placed (the callables are not).
        assert_eq!(u.graph.node_count(), 3, "system + two containers");

        // The system is the sole containment root, holding both containers.
        assert_eq!(u.roots.len(), 1);
        let sys = u.roots[0];
        assert_eq!(u.graph[sys].level, C4Level::System);
        assert_eq!(u.graph[sys].children.len(), 2);

        // The call App.Do -> Svc.Handle lifts to a single App<->Svc relationship.
        assert_eq!(u.graph.edge_count(), 1);

        // Every placed node's level is structural, and ids are the model FQNs.
        for nx in u.graph.node_indices() {
            let n = &u.graph[nx];
            assert!(matches!(n.level, C4Level::System | C4Level::Container));
            assert!(n.id.starts_with("m::"));
        }
    }

    #[test]
    fn an_empty_model_yields_an_empty_universe() {
        let u = build(&[]);
        assert_eq!(u.graph.node_count(), 0);
        assert!(u.roots.is_empty());
    }
}
