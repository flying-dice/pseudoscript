//! Visual-layout optimisation: reduce long edges.
//!
//! Network simplex already minimises *total* edge length, but a single dominant
//! long edge (a feedback edge spanning the whole diagram) reads worse than
//! several short ones of the same total. This pass searches **same-rank moves**
//! that lower the *sum of squared* edge lengths — an objective that punishes long
//! edges superlinearly — and returns the [`crate::Graph::same_rank`] groups that
//! achieve it. Greedy and bounded: at most [`MAX_ROUNDS`] rounds, each trying the
//! [`TOP_K`] longest edges; deterministic (stable tie-breaks throughout).

use crate::graph::Graph;

/// Maximum greedy rounds (each accepts at most one same-rank move).
const MAX_ROUNDS: usize = 4;
/// How many of the longest edges to attempt a move on each round.
const TOP_K: usize = 3;
/// Minimum score improvement (points²) to accept a move — avoids churn.
const EPS: f64 = 1.0;

/// Search same-rank groups that minimise Σ edge-length². Returns groups to set on
/// [`Graph::same_rank`] (including any already present).
#[must_use]
pub fn minimize_long_edges(graph: &Graph) -> Vec<Vec<String>> {
    let mut groups = graph.same_rank.clone();
    for _ in 0..MAX_ROUNDS {
        let base_score = score(&with_groups(graph, &groups));
        let ranks = ranks_of(&with_groups(graph, &groups));

        // Longest edges first (by rank span), stable by index.
        let mut by_span: Vec<usize> = (0..graph.edges.len()).collect();
        by_span.sort_by(|&a, &b| {
            span(graph, &ranks, b)
                .cmp(&span(graph, &ranks, a))
                .then(a.cmp(&b))
        });

        let mut best: Option<(f64, Vec<Vec<String>>)> = None;
        for &ei in by_span.iter().take(TOP_K) {
            if span(graph, &ranks, ei) < 2 {
                continue;
            }
            let e = &graph.edges[ei];
            let (Some(ti), Some(hi)) = (graph.node_index(&e.tail), graph.node_index(&e.head))
            else {
                continue;
            };
            let deep = if ranks[ti] > ranks[hi] { ti } else { hi };
            let (lo, hi_rank) = (ranks[ti].min(ranks[hi]), ranks[ti].max(ranks[hi]));

            // Try aligning the deep endpoint with a node on an intermediate rank.
            // `w` is a node index used across `ranks`/`nodes`/`deep`, not a plain
            // slice cursor.
            #[allow(clippy::needless_range_loop)]
            for w in 0..graph.nodes.len() {
                if w == deep || ranks[w] <= lo || ranks[w] >= hi_rank {
                    continue;
                }
                // Never same-rank two directly-connected nodes — it makes a flat
                // edge, which reads poorly and adds no length saving.
                if connected(graph, deep, w) {
                    continue;
                }
                let cand = union_pair(&groups, &graph.nodes[deep].id, &graph.nodes[w].id);
                let s = score(&with_groups(graph, &cand));
                if s + EPS < base_score && best.as_ref().is_none_or(|(bs, _)| s < *bs) {
                    best = Some((s, cand));
                }
            }
        }

        match best {
            Some((_, g)) => groups = g,
            None => break,
        }
    }
    groups
}

/// `graph` cloned with `same_rank` set to `groups`.
fn with_groups(graph: &Graph, groups: &[Vec<String>]) -> Graph {
    let mut g = graph.clone();
    g.same_rank = groups.to_vec();
    g
}

/// Σ over edges of the squared distance between endpoint centres — the objective.
fn score(graph: &Graph) -> f64 {
    let layout = crate::layout(graph);
    let mut total = 0.0;
    for e in &layout.edges {
        let (Some(t), Some(h)) = (
            layout.nodes.iter().find(|n| n.id == e.tail),
            layout.nodes.iter().find(|n| n.id == e.head),
        ) else {
            continue;
        };
        let dx = t.center.x - h.center.x;
        let dy = t.center.y - h.center.y;
        total += dx * dx + dy * dy;
    }
    total
}

/// The rank of every node (index-aligned to `graph.nodes`).
fn ranks_of(graph: &Graph) -> Vec<i32> {
    crate::rank::assign_ranks(graph).rank
}

/// Whether an edge directly connects node indices `a` and `b` (either direction).
fn connected(graph: &Graph, a: usize, b: usize) -> bool {
    let (ida, idb) = (&graph.nodes[a].id, &graph.nodes[b].id);
    graph
        .edges
        .iter()
        .any(|e| (&e.tail == ida && &e.head == idb) || (&e.tail == idb && &e.head == ida))
}

/// The rank span of edge `ei` (absolute difference; 0 for an unresolved edge).
fn span(graph: &Graph, ranks: &[i32], ei: usize) -> i32 {
    let e = &graph.edges[ei];
    match (graph.node_index(&e.tail), graph.node_index(&e.head)) {
        (Some(t), Some(h)) => (ranks[t] - ranks[h]).abs(),
        _ => 0,
    }
}

/// `groups` with `a` and `b` placed in the same group (merging existing groups
/// that already contain either).
fn union_pair(groups: &[Vec<String>], a: &str, b: &str) -> Vec<Vec<String>> {
    let mut merged: Vec<String> = vec![a.to_owned(), b.to_owned()];
    let mut rest: Vec<Vec<String>> = Vec::new();
    for g in groups {
        if g.iter().any(|m| m == a || m == b) {
            merged.extend(g.iter().cloned());
        } else {
            rest.push(g.clone());
        }
    }
    merged.sort();
    merged.dedup();
    rest.push(merged);
    rest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Node};

    /// The motivating shape: a chain a→b→c→d with a feedback edge d→a. The
    /// feedback edge spans the whole chain; the optimiser should same-rank `d`
    /// higher to cut Σlength².
    fn feedback_graph() -> Graph {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(Node::new(id, 60.0, 30.0));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("b", "c"));
        g.edges.push(Edge::new("c", "d"));
        g.edges.push(Edge::new("d", "a")); // feedback
        g
    }

    #[test]
    fn lowers_sum_of_squared_lengths() {
        let g = feedback_graph();
        let before = score(&g);
        let groups = minimize_long_edges(&g);
        assert!(!groups.is_empty(), "found at least one same-rank move");
        let after = score(&with_groups(&g, &groups));
        assert!(after < before, "Σlen² reduced: {before} -> {after}");
    }

    #[test]
    fn deterministic() {
        let g = feedback_graph();
        assert_eq!(minimize_long_edges(&g), minimize_long_edges(&g));
    }

    #[test]
    fn no_long_edges_leaves_groups_unchanged() {
        // A simple chain has no edge worth moving.
        let mut g = Graph::new();
        for id in ["a", "b"] {
            g.nodes.push(Node::new(id, 60.0, 30.0));
        }
        g.edges.push(Edge::new("a", "b"));
        assert!(minimize_long_edges(&g).is_empty());
    }
}
