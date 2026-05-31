//! Flowchart layout — **extension point, not yet implemented.**
//!
//! A flowchart is a directed graph of nodes (steps, decisions) and edges,
//! typically laid out in ranked layers. The engine will live here and implement
//! [`crate::Projection`] over a `flowchart::Diagram` (nodes + directed edges),
//! producing placed node rectangles and routed edges — reusing
//! [`crate::geom`] for rectangles and [`crate::text`] for node label widths.
