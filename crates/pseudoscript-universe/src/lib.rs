//! The **software graph** model for the 3D relationship view.
//!
//! Maps the resolved C4 model into a graph the 3D renderers (the web IDE's
//! `ForceGraph` and the doc site's universe page) draw: structural nodes
//! (systems, containers, components, people) with the containment tree,
//! directed relationship edges weighted by traffic, and the entry-point flows
//! traced through the model's sequence projections. Positions are NOT computed
//! here: the renderer lays the graph out with d3-force-3d in the browser. Pure
//! and wasm-safe; no rendering, no layout.

mod flows;
mod model_adapter;
mod snapshot;

pub use flows::{FlowDef, FlowHop, flows};
pub use model_adapter::{C4Level, GraphNode, NodeIx, Universe, build, from_model};
pub use snapshot::{EdgeOut, NodeOut, Snapshot, snapshot};
