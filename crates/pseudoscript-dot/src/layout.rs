//! The positioned result of [`crate::layout`]: node centres, routed edge
//! splines, cluster boxes, and the overall bounding box. All coordinates are in
//! points, y growing downward (see [`crate::graph`]).

use serde::{Deserialize, Serialize};

/// A point in points (y-down).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    #[must_use]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned box in points (y-down): `(x, y)` is the top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Box2 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Box2 {
    #[must_use]
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// A placed node: its centre and the size it was given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodePos {
    /// The node id.
    pub id: String,
    /// Centre of the node, in points.
    pub center: Pt,
    /// Width in points (unchanged from input).
    pub width: f64,
    /// Height in points (unchanged from input).
    pub height: f64,
}

/// A routed edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeRoute {
    /// Source node id.
    pub tail: String,
    /// Target node id.
    pub head: String,
    /// Piecewise cubic Bézier control points: `1 + 3k` points for `k` segments
    /// (`P0`, then `(C1, C2, P)` per segment), running tail → head.
    pub spline: Vec<Pt>,
    /// The spline flattened to a dense polyline, tail → head — what a polyline
    /// renderer draws directly. Endpoints coincide with the spline's.
    pub polyline: Vec<Pt>,
    /// Where to place the edge label, if the edge carried one.
    pub label_pos: Option<Pt>,
}

/// A cluster's bounding box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterBox {
    /// The cluster id.
    pub id: String,
    /// The bounding box enclosing the cluster's members plus its margin.
    pub bbox: Box2,
}

/// The grid the experimental placement laid nodes on — enough for a client to map
/// a pixel position back to a cell (drag-to-pin). `Some` only for a grid layout.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridMeta {
    /// Columns and rows in the grid.
    pub cols: usize,
    pub rows: usize,
    /// Cell pitch in points (centre-to-centre spacing).
    pub cell_w: f64,
    pub cell_h: f64,
    /// Pixel centre of cell `(row 0, col 0)`; cell `(r, c)` centres at
    /// `origin + (c·cell_w, r·cell_h)`.
    pub origin: Pt,
}

/// The full positioned layout.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Layout {
    /// The overall bounding box enclosing every node, edge, and cluster.
    pub bbox: Box2,
    /// Placed nodes, in input order.
    pub nodes: Vec<NodePos>,
    /// Routed edges, in input order.
    pub edges: Vec<EdgeRoute>,
    /// Cluster boxes, in input order.
    pub clusters: Vec<ClusterBox>,
    /// The grid geometry, when this came from [`crate::grid_layout`]; else `None`.
    #[serde(default)]
    pub grid: Option<GridMeta>,
}
