//! C4 structural layout lives in the dedicated `pseudoscript-dot` crate — a
//! pure-Rust port of the Graphviz `dot` layered engine (rank / order / position
//! / splines, with the boundary modelled as a cluster). `pseudoscript-emit`
//! (`c4_render.rs`) maps a C4 scene to a `pseudoscript_dot::Graph` and back.
//!
//! C4 needs `dot`'s hierarchical ranks, crossing minimisation, and cluster
//! boxes, which this crate's sequence-style engines do not provide; keeping the
//! port a standalone crate makes it reusable and independently testable against
//! the real `dot` binary. This module is intentionally empty.
