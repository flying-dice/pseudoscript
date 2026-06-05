//! The 3D **software universe** layout engine.
//!
//! Positions every C4 element in 3D space for the exploratory universe view. The
//! work is a *compound* (nested) graph layout — Systems contain Containers contain
//! Components — solved recursively per cluster level, NOT a flat force simulation
//! (handover spec §3). This crate is deliberately isolated and pure: no rendering,
//! no threading assumptions, and its public API never leaks the force-engine's
//! types, so the (future, AGPL) simulation dependency stays confined here (§12) and
//! the whole crate compiles to both native and `wasm32`.
//!
//! **Phase 1 (this code):** the model adapter only — map the existing resolved C4
//! model into the universe's internal graph ([`Universe`]). No simulation yet; the
//! `pos`/`radius` outputs are computed in later phases.

mod model_adapter;

pub use model_adapter::{C4Level, LayoutNode, NodeIx, Universe, build, from_model};
