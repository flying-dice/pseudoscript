//! A composable post-layout pipeline.
//!
//! [`crate::layout`] produces the base `dot` placement; a pipeline of [`Pass`]es
//! then refines it. Every pass has the **same shape** — [`LayoutState`] in,
//! [`LayoutState`] out — so passes compose freely: the output of one is the input
//! to the next, and the set of passes is just a slice run in order.
//!
//! A pass is free to either adjust the geometry directly (move nodes, reroute
//! edges) or change the *input* graph — for instance add `same_rank` hints — and
//! re-run the base layout via [`LayoutState::relayout`]. Both kinds compose the
//! same way, so new behaviour is added by writing a `Pass`, not by threading more
//! flags through the engine.

use crate::graph::Graph;
use crate::layout::Layout;

/// The structure that flows through a pipeline: the (possibly pass-mutated) input
/// graph and its current placement. A pass receives it, returns a new one, and
/// the framework hands that to the next pass.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutState {
    /// The layout input. A pass may mutate it (e.g. add `same_rank` groups) and
    /// then call [`LayoutState::relayout`].
    pub graph: Graph,
    /// The current placement.
    pub layout: Layout,
}

impl LayoutState {
    /// Lay `graph` out from scratch, the pipeline's starting state.
    #[must_use]
    pub fn base(graph: Graph) -> Self {
        let layout = crate::layout(&graph);
        Self { graph, layout }
    }

    /// Re-run the base `dot` layout for the current graph, replacing the
    /// placement — what a re-ranking pass calls after editing `graph`.
    pub fn relayout(&mut self) {
        self.layout = crate::layout(&self.graph);
    }
}

/// One stage of a layout pipeline: same structure in, same structure out.
///
/// Implement this for a new refinement (alignment, compaction, edge re-routing,
/// short-edge hinting, …); it then drops into any pipeline.
pub trait Pass {
    /// A short identifier, for debugging and telemetry.
    fn name(&self) -> &'static str;

    /// Refine `state` and return the result.
    fn run(&self, state: LayoutState) -> LayoutState;
}

/// Lay `graph` out with `dot`, then fold the result through `passes` in order.
///
/// Deterministic and total: with no passes it is exactly [`crate::layout`].
#[must_use]
pub fn run_pipeline(graph: &Graph, passes: &[&dyn Pass]) -> Layout {
    let mut state = LayoutState::base(graph.clone());
    for pass in passes {
        state = pass.run(state);
    }
    state.layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Node};

    fn chain() -> Graph {
        let mut g = Graph::new();
        for id in ["a", "b", "c"] {
            g.nodes.push(Node::new(id, 60.0, 30.0));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("b", "c"));
        g
    }

    /// A no-op pass that leaves the state untouched.
    struct Identity;
    impl Pass for Identity {
        fn name(&self) -> &'static str {
            "identity"
        }
        fn run(&self, state: LayoutState) -> LayoutState {
            state
        }
    }

    /// A pass that shifts every node right by a fixed amount — proves geometry
    /// edits flow through and compose.
    struct ShiftRight(f64);
    impl Pass for ShiftRight {
        fn name(&self) -> &'static str {
            "shift-right"
        }
        fn run(&self, mut state: LayoutState) -> LayoutState {
            for n in &mut state.layout.nodes {
                n.center.x += self.0;
            }
            state
        }
    }

    #[test]
    fn empty_pipeline_equals_base_layout() {
        let g = chain();
        assert_eq!(run_pipeline(&g, &[]), crate::layout(&g));
    }

    #[test]
    fn passes_compose_in_order() {
        let g = chain();
        let base = crate::layout(&g);
        let out = run_pipeline(&g, &[&ShiftRight(10.0), &Identity, &ShiftRight(5.0)]);
        for (b, o) in base.nodes.iter().zip(&out.nodes) {
            assert!(
                (o.center.x - (b.center.x + 15.0)).abs() < 1e-9,
                "shifts accumulate"
            );
        }
    }
}
