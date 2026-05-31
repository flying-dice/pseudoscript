//! C4 structural layout — **extension point, not yet implemented.**
//!
//! A C4 view (context / container / component) places nodes and routes edges
//! within an optional boundary box. The engine will live here and implement
//! [`crate::Projection`] over a `c4::Diagram` (nodes + edges + boundary),
//! producing a `c4::Layout` of placed node rectangles and routed edge paths —
//! reusing [`crate::geom`] (`Rect`, `Bounds` for the boundary) and
//! [`crate::text`] (node title widths).
//!
//! Today `pseudoscript-emit` lays C4 out via the external `layout-rs` crate;
//! that responsibility moves here so every projection shares one layout home.
