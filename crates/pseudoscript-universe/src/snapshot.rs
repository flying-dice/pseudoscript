//! A flat, serialisable snapshot of the software graph — the contract the web IDE's
//! `ForceGraph` consumes: each node's id, C4 level, and containment parent, plus the
//! directed relationships weighted by traffic and coloured by the destination's
//! archetype. Positions are not here — the renderer lays the graph out client-side.
//! No engine internals (petgraph indices) leak across this boundary.

use serde::Serialize;

use crate::model_adapter::{C4Level, NodeIx, Universe};
use crate::personality::Archetype;

/// One node.
#[derive(Debug, Clone, Serialize)]
pub struct NodeOut {
    pub id: String,
    /// `"system"` | `"container"` | `"component"` | `"person"`.
    pub level: &'static str,
    /// Enclosing node's id (`None` for a top-level system).
    pub parent: Option<String>,
}

/// One relationship, with its traffic (call count) and pattern.
#[derive(Debug, Clone, Serialize)]
pub struct EdgeOut {
    pub from: String,
    pub to: String,
    pub traffic: u32,
    /// The archetype of the destination node (`"gateway"`, `"pulsar"`, `"storm"`,
    /// `"world"`, …) — the kind of world this traffic feeds; colours the flow.
    pub kind: &'static str,
}

/// The whole graph, ready to render.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub nodes: Vec<NodeOut>,
    pub edges: Vec<EdgeOut>,
}

/// Flatten the software graph into a renderer-facing [`Snapshot`].
#[must_use]
pub fn snapshot(universe: &Universe) -> Snapshot {
    let id_of = |nx: NodeIx| universe.graph[nx].id.clone();

    let nodes = universe
        .graph
        .node_indices()
        .map(|nx| {
            let n = &universe.graph[nx];
            NodeOut {
                id: n.id.clone(),
                level: level_str(n.level),
                parent: n.parent.map(id_of),
            }
        })
        .collect();

    let edges = universe
        .graph
        .edge_indices()
        .filter_map(|e| {
            let (a, b) = universe.graph.edge_endpoints(e)?;
            Some(EdgeOut {
                from: id_of(a),
                to: id_of(b),
                traffic: universe.graph[e],
                // The flow takes the colour of the world it feeds — its archetype
                // (macro-derived: pulsar/storm/gateway/forge/…), so traffic of
                // different kinds reads as different colours.
                kind: archetype_str(universe.graph[b].planet.archetype),
            })
        })
        .collect();

    Snapshot { nodes, edges }
}

fn level_str(level: C4Level) -> &'static str {
    match level {
        C4Level::System => "system",
        C4Level::Container => "container",
        C4Level::Component => "component",
        C4Level::Person => "person",
    }
}

fn archetype_str(a: Archetype) -> &'static str {
    match a {
        Archetype::Star => "star",
        Archetype::Beacon => "beacon",
        Archetype::Pulsar => "pulsar",
        Archetype::Storm => "storm",
        Archetype::Gateway => "gateway",
        Archetype::Forge => "forge",
        Archetype::World => "world",
        Archetype::Tomb => "tomb",
    }
}
