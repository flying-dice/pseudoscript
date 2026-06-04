//! The input graph: nodes with fixed sizes, directed edges, and optional
//! clusters, mirroring the subset of Graphviz `dot` attributes this engine
//! honours. A client builds a [`Graph`] directly (there is no DOT parser) and
//! hands it to [`crate::layout`].
//!
//! Coordinates are in **points** (Graphviz's unit, 1/72 inch) with **y growing
//! downward**, origin top-left — the screen convention the renderer consumes.
//! (Graphviz internally uses y-up; the only place that matters is oracle
//! comparison, which flips one axis. The rank/order/x-position algorithms are
//! coordinate-direction agnostic; only the final y-assignment differs.)

use serde::{Deserialize, Serialize};

/// Rank growth direction. `dot`'s `rankdir`. Ranks increase along the major
/// axis: down for [`RankDir::TopBottom`], right for [`RankDir::LeftRight`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RankDir {
    /// Top to bottom (ranks increase in +y). The default, and what C4 uses.
    #[default]
    TopBottom,
    /// Left to right (ranks increase in +x).
    LeftRight,
}

/// A node with a fixed size. The engine never resizes a node; it only places it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier (the client's key; opaque to the engine).
    pub id: String,
    /// Width in points.
    pub width: f64,
    /// Height in points.
    pub height: f64,
}

impl Node {
    /// A node with the given id and size.
    #[must_use]
    pub fn new(id: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            id: id.into(),
            width,
            height,
        }
    }
}

/// A directed edge from [`Edge::tail`] to [`Edge::head`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source node id.
    pub tail: String,
    /// Target node id.
    pub head: String,
    /// Minimum rank difference `rank(head) - rank(tail)` (`dot` `minlen`,
    /// default 1).
    pub minlen: i32,
    /// Layout weight: higher pulls the endpoints into vertical alignment harder
    /// (`dot` `weight`, default 1).
    pub weight: i32,
    /// Optional edge-label size, reserved as space in the layout (`dot` routes
    /// the edge around a virtual label node). `None` for an unlabelled edge.
    pub label: Option<(f64, f64)>,
}

impl Edge {
    /// An edge with `minlen = 1`, `weight = 1`, no label.
    #[must_use]
    pub fn new(tail: impl Into<String>, head: impl Into<String>) -> Self {
        Self {
            tail: tail.into(),
            head: head.into(),
            minlen: 1,
            weight: 1,
            label: None,
        }
    }
}

/// A cluster: a set of nodes laid out together within a bounding box, kept on
/// contiguous ranks and separated from the rest of the graph (`dot`'s
/// `subgraph cluster_*`). C4 uses exactly one cluster (the boundary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cluster {
    /// Unique cluster id.
    pub id: String,
    /// Ids of the member nodes.
    pub members: Vec<String>,
    /// Padding in points between the cluster's contents and its bounding box.
    pub margin: f64,
}

/// The input graph handed to [`crate::layout`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    /// Nodes, in a stable client-defined order (used as the deterministic
    /// tie-break throughout the pipeline).
    pub nodes: Vec<Node>,
    /// Directed edges.
    pub edges: Vec<Edge>,
    /// Clusters (may be empty).
    pub clusters: Vec<Cluster>,
    /// Rank direction.
    pub rankdir: RankDir,
    /// Minimum gap between adjacent nodes within a rank, in points (`dot`
    /// `nodesep`, default 18 = 0.25in).
    pub nodesep: f64,
    /// Minimum gap between adjacent ranks, in points (`dot` `ranksep`, default
    /// 36 = 0.5in).
    pub ranksep: f64,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            rankdir: RankDir::default(),
            nodesep: 18.0,
            ranksep: 36.0,
        }
    }
}

impl Graph {
    /// An empty graph with `dot`'s default spacing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The index of the node with id `id`, if present.
    #[must_use]
    pub fn node_index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }
}
