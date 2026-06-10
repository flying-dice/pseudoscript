//! Adapt the resolved `PseudoScript` C4 model into the relationship graph.
//!
//! Keeps only the **structural** nodes — systems, containers, components, people.
//! Data and callables are not nodes; a relationship that runs through them is
//! *lifted* to the nearest enclosing structural node, so the graph carries
//! node-to-node relationships at any level (spec §4, §7). Containment lives on each
//! node (`parent`/`children`); relationships are the petgraph edges. No positions —
//! the renderer lays the graph out client-side.

use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};
use pseudoscript_model::{
    EdgeKind, Graph as Model, NodeKind, WorkspaceModule, graph as build_model,
};

/// An index into the graph (petgraph's node index).
pub type NodeIx = NodeIndex<u32>;

/// The C4 abstraction level a node sits at. Systems, containers, components, and the
/// people (actors) who use them enter the graph; data and callables do not
/// (relationships through them are lifted to the nearest of these).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4Level {
    System,
    Container,
    Component,
    Person,
}

/// A node in the software graph: a system, container, component, or person, with its
/// place in the containment tree. Positions are not here — the renderer lays the
/// graph out client-side.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Stable model FQN.
    pub id: String,
    pub level: C4Level,
    /// Enclosing node in the containment tree (`None` for a top-level system).
    pub parent: Option<NodeIx>,
    pub children: Vec<NodeIx>,
}

impl GraphNode {
    fn new(id: String, level: C4Level) -> Self {
        Self {
            id,
            level,
            parent: None,
            children: Vec::new(),
        }
    }
}

/// The software graph: structural nodes (petgraph weights) with the containment tree
/// on each node, directed relationship edges (weighted by traffic) between them, and
/// the top-level systems.
pub struct Universe {
    /// Nodes are [`GraphNode`]; each edge weight is its **traffic** — the number
    /// of underlying calls between the two nodes.
    pub graph: Graph<GraphNode, u32>,
    /// Top-level systems (containment roots), in model declaration order.
    pub roots: Vec<NodeIx>,
}

/// Build the graph from module sources — the same `(fqn, source)` input the IDE feeds
/// [`pseudoscript_model::graph`].
#[must_use]
pub fn build(modules: &[WorkspaceModule]) -> Universe {
    from_model(&build_model(modules))
}

/// Adapt an already-resolved model into the software graph.
#[must_use]
pub fn from_model(model: &Model) -> Universe {
    let mut graph: Graph<GraphNode, u32> = Graph::new();

    // 1. Each structural model node becomes a graph node; remember its index.
    let mut ix: HashMap<&str, NodeIx> = HashMap::new();
    for node in model.nodes() {
        if let Some(level) = level_of(node.kind) {
            let nx = graph.add_node(GraphNode::new(node.fqn.clone(), level));
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

    // 3. Relationships: lift every call to the structural level and tally traffic per
    //    *directed* pair (caller → callee), so the edge carries direction — the flow
    //    animation in the renderer streams along it. The count becomes the weight.
    let mut traffic: HashMap<(NodeIx, NodeIx), u32> = HashMap::new();
    for edge in model.edges() {
        if edge.kind != EdgeKind::Call {
            continue;
        }
        let (Some(a), Some(b)) = (lift(model, &edge.from, &ix), lift(model, &edge.to, &ix)) else {
            continue;
        };
        if a != b {
            *traffic.entry((a, b)).or_default() += 1;
        }
    }
    // Add edges in a stable order (HashMap iteration is randomised) so the snapshot
    // is identical every run — determinism (spec §8).
    let mut pairs: Vec<((NodeIx, NodeIx), u32)> = traffic.iter().map(|(&k, &v)| (k, v)).collect();
    pairs.sort_by_key(|((a, b), _)| (a.index(), b.index()));
    for ((a, b), weight) in pairs {
        graph.add_edge(a, b, weight);
    }

    Universe { graph, roots }
}

/// The structural level of a model kind, or `None` for the kinds the graph does not
/// place (data, callable).
fn level_of(kind: NodeKind) -> Option<C4Level> {
    match kind {
        NodeKind::System => Some(C4Level::System),
        NodeKind::Container => Some(C4Level::Container),
        NodeKind::Component => Some(C4Level::Component),
        NodeKind::Person => Some(C4Level::Person),
        NodeKind::Data | NodeKind::Callable => None,
    }
}

/// Walk a model FQN up its `parent` chain to the nearest structural node present in
/// the graph; `None` if the chain reaches the top without one.
fn lift(model: &Model, fqn: &str, ix: &HashMap<&str, NodeIx>) -> Option<NodeIx> {
    lift_fqn(model, fqn, |cur| ix.contains_key(cur)).and_then(|cur| ix.get(cur).copied())
}

/// Walk a model FQN up its `parent` chain to the nearest FQN `is_placed`
/// accepts — the one lift relationships and flows share, so the two always
/// agree on where a call lands.
pub(crate) fn lift_fqn<'m>(
    model: &'m Model,
    fqn: &'m str,
    is_placed: impl Fn(&str) -> bool,
) -> Option<&'m str> {
    let mut cur = fqn;
    loop {
        if is_placed(cur) {
            return Some(cur);
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

    #[test]
    fn calls_tally_directed_traffic_and_self_calls_are_dropped() {
        // App calls Svc twice and itself once; the self-call is not an edge, and the
        // two App→Svc calls collapse to one directed edge of weight 2.
        let src = "\
system Sys;
container App for m::Sys {
  Do() {
    m::Svc.Handle()
    m::Svc.Handle()
    self.Again()
  }
  Again();
}
container Svc for m::Sys {
  Handle();
}
";
        let u = build(&module(src));
        assert_eq!(u.graph.edge_count(), 1, "one App→Svc edge, no self-edge");
        assert_eq!(u.graph.edge_weights().copied().max(), Some(2), "two calls");
    }
}
