//! The **software graph** model for the 3D relationship view.
//!
//! Maps the resolved C4 model into a graph the web IDE's `ForceGraph` renders:
//! structural nodes (systems, containers, components, people) with the containment
//! tree, directed relationship edges weighted by traffic, and each node's
//! macro-derived personality ([`Archetype`]) — which colours the traffic. Positions
//! are NOT computed here: the renderer lays the graph out with d3-force-3d in the
//! browser. Pure and wasm-safe; no rendering, no layout.

mod model_adapter;
mod personality;
mod snapshot;

pub use model_adapter::{
    C4Level, Freshness, LayoutNode, NodeIx, Universe, build, build_with, from_model,
    from_model_with,
};
pub use personality::{Archetype, Planet};
pub use snapshot::{EdgeOut, NodeOut, Snapshot, snapshot};
