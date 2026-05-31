//! Diagram layout — positional algorithms for `PseudoScript`'s projections,
//! independent of how a diagram is produced or drawn.
//!
//! The crate is a shared geometry/text core ([`geom`], [`text`]) plus one module
//! per diagram kind. Each engine turns a structural input into absolute
//! coordinates a consumer renders verbatim, so layout lives in exactly one
//! place, is unit-testable in isolation, and stays identical across the static
//! SVG renderer and the web-ide.
//!
//! [`sequence`] is implemented; [`c4`] and [`flowchart`] are the next engines,
//! built on the same core via the [`Projection`] contract.

pub mod c4;
pub mod flowchart;
pub mod geom;
pub mod sequence;
pub mod text;

pub use geom::{Bounds, Point, Rect, Size};
pub use text::TextMetrics;

/// The contract every projection's layout engine implements: turn a structural
/// `Input` into a positioned `Output`, parameterised by projection-specific
/// `Metrics`. An associated-type trait (not `dyn`) — engines stay monomorphic
/// and zero-cost while sharing one well-known shape.
///
/// Implemented by [`sequence::Sequence`]; future engines (`c4`, `flowchart`)
/// implement it the same way.
pub trait Projection {
    /// The structural diagram description.
    type Input;
    /// Tunable spacing/text metrics for this projection.
    type Metrics;
    /// The positioned result.
    type Output;

    /// Lay `input` out under `metrics`.
    fn layout(input: &Self::Input, metrics: &Self::Metrics) -> Self::Output;
}
