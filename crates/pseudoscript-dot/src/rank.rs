//! Rank assignment — `dot`'s first major pass (`lib/dotgen/rank.c`), with
//! cluster banding from `lib/dotgen/cluster.c`.
//!
//! Cycles are broken ([`crate::acyclic`]); each edge becomes a difference
//! constraint solved by network simplex ([`crate::ns`]). Clusters are laid out
//! by **collapse/expand**: a cluster is ranked internally on its own, replaced
//! in the parent graph by a top/bottom skeleton pair spanning its rank band,
//! the parent is ranked, then internal ranks are offset by the band top. The
//! effect is that a cluster's members occupy contiguous ranks and every node
//! outside the cluster is ranked strictly above or below the band — the C4
//! "externals outside the boundary" guarantee, produced by the layout rather
//! than patched in afterwards.

use crate::graph::Graph;
use crate::ns::{Balance, Constraint, rank};

/// The result of ranking: a rank per node and the reversed-edge flags from cycle
/// breaking (both indexed like [`Graph::nodes`] / [`Graph::edges`]).
#[derive(Debug, Clone)]
pub(crate) struct Ranking {
    pub rank: Vec<i32>,
    // Consumed by spline routing (Phase 5): a reversed edge is routed on the
    // flipped chain and its polyline reversed so the arrowhead lands on the true
    // head.
    #[allow(dead_code)]
    pub reversed: Vec<bool>,
}

/// Assign a rank to every node in `graph`.
pub(crate) fn assign_ranks(graph: &Graph) -> Ranking {
    let n = graph.nodes.len();
    if n == 0 {
        return Ranking {
            rank: Vec::new(),
            reversed: vec![false; graph.edges.len()],
        };
    }

    // Resolve edge endpoints to node indices once; unknown ids drop out.
    let ends: Vec<Option<(usize, usize)>> = graph
        .edges
        .iter()
        .map(|e| Some((graph.node_index(&e.tail)?, graph.node_index(&e.head)?)))
        .collect();

    // Break cycles over the resolved edges (unknown-id edges contribute none).
    let resolved: Vec<(usize, usize)> = ends.iter().map(|o| o.unwrap_or((0, 0))).collect();
    let mut reversed = crate::acyclic::break_cycles(n, &resolved);
    for (i, end) in ends.iter().enumerate() {
        if end.is_none() {
            reversed[i] = false;
        }
    }

    // Effective layering edges: reversed edges point head → tail; self-loops and
    // unknown-id edges are dropped (no rank constraint).
    let mut eff: Vec<Edge> = Vec::new();
    for (i, end) in ends.iter().enumerate() {
        let Some((t, h)) = *end else { continue };
        if t == h {
            continue;
        }
        let (tail, head) = if reversed[i] { (h, t) } else { (t, h) };
        eff.push(Edge {
            tail,
            head,
            minlen: graph.edges[i].minlen.max(1),
            weight: graph.edges[i].weight.max(1),
        });
    }

    let cluster_of = cluster_membership(graph, n);
    let rank_vec = if graph.clusters.is_empty() {
        let cs: Vec<Constraint> = eff.iter().map(Edge::constraint).collect();
        rank(n, &cs, Balance::TopBottom)
    } else {
        rank_with_clusters(graph, n, &eff, &cluster_of)
    };

    Ranking {
        rank: rank_vec,
        reversed,
    }
}

/// An effective (post-acyclic) layering edge between node indices.
#[derive(Debug, Clone, Copy)]
struct Edge {
    tail: usize,
    head: usize,
    minlen: i32,
    weight: i32,
}

impl Edge {
    fn constraint(&self) -> Constraint {
        Constraint {
            tail: self.tail,
            head: self.head,
            minlen: self.minlen,
            weight: self.weight,
        }
    }
}

/// `cluster_of[v]` = the cluster index containing node `v`, or `None` if free.
/// A node named in several clusters takes the first (clusters are disjoint in
/// well-formed input).
fn cluster_membership(graph: &Graph, n: usize) -> Vec<Option<usize>> {
    let mut of = vec![None; n];
    for (ci, c) in graph.clusters.iter().enumerate() {
        for m in &c.members {
            if let Some(v) = graph.node_index(m)
                && of[v].is_none()
            {
                of[v] = Some(ci);
            }
        }
    }
    of
}

/// Rank a graph with clusters by collapse/expand (single level, disjoint
/// clusters — what C4 needs).
fn rank_with_clusters(
    graph: &Graph,
    n: usize,
    eff: &[Edge],
    cluster_of: &[Option<usize>],
) -> Vec<i32> {
    let num_clusters = graph.clusters.len();

    // 1. Rank each cluster internally on its own member subgraph.
    let mut internal = vec![0i32; n]; // internal rank within the owning cluster
    let mut band_height = vec![0i32; num_clusters]; // maxC per cluster
    // `ci` is the cluster identity, used for membership tests throughout — not a
    // plain slice index, so range iteration reads clearer than `enumerate`.
    #[allow(clippy::needless_range_loop)]
    for ci in 0..num_clusters {
        let members: Vec<usize> = (0..n).filter(|&v| cluster_of[v] == Some(ci)).collect();
        if members.is_empty() {
            continue;
        }
        let local: std::collections::HashMap<usize, usize> =
            members.iter().enumerate().map(|(i, &v)| (v, i)).collect();
        let cs: Vec<Constraint> = eff
            .iter()
            .filter(|e| cluster_of[e.tail] == Some(ci) && cluster_of[e.head] == Some(ci))
            .map(|e| Constraint {
                tail: local[&e.tail],
                head: local[&e.head],
                minlen: e.minlen,
                weight: e.weight,
            })
            .collect();
        let r = rank(members.len(), &cs, Balance::None);
        for (i, &v) in members.iter().enumerate() {
            internal[v] = r[i];
        }
        band_height[ci] = r.iter().copied().max().unwrap_or(0);
    }

    // 2. Build the collapsed parent problem: free nodes keep an id; each cluster
    //    contributes a top and bottom skeleton node.
    let num_free = (0..n).filter(|&v| cluster_of[v].is_none()).count();
    let mut free_pid = vec![usize::MAX; n];
    let mut next = 0;
    for v in 0..n {
        if cluster_of[v].is_none() {
            free_pid[v] = next;
            next += 1;
        }
    }
    let top_pid = |ci: usize| num_free + 2 * ci;
    let bottom_pid = |ci: usize| num_free + 2 * ci + 1;
    let parent_n = num_free + 2 * num_clusters;

    // Endpoint in the parent graph for an edge entering / leaving node `v`.
    let enter = |v: usize| cluster_of[v].map_or(free_pid[v], top_pid); // a head uses the band top
    let leave = |v: usize| cluster_of[v].map_or(free_pid[v], bottom_pid); // a tail uses the band bottom

    let mut pc: Vec<Constraint> = Vec::new();
    for e in eff {
        match (cluster_of[e.tail], cluster_of[e.head]) {
            (Some(a), Some(b)) if a == b => {} // internal: handled in step 1
            _ => pc.push(Constraint {
                tail: leave(e.tail),
                head: enter(e.head),
                minlen: e.minlen,
                weight: e.weight,
            }),
        }
    }
    // Band-height edges: top → bottom with minlen = the cluster's internal height.
    for (ci, &bh) in band_height.iter().enumerate() {
        pc.push(Constraint {
            tail: top_pid(ci),
            head: bottom_pid(ci),
            minlen: bh,
            weight: 1,
        });
    }

    let parent_rank = rank(parent_n, &pc, Balance::None);

    // 3. Expand: free node = its parent rank; member = band top + internal rank.
    let mut out = vec![0i32; n];
    for v in 0..n {
        out[v] = match cluster_of[v] {
            None => parent_rank[free_pid[v]],
            Some(ci) => parent_rank[top_pid(ci)] + internal[v],
        };
    }
    // Normalise to min 0.
    if let Some(&min) = out.iter().min() {
        for r in &mut out {
            *r -= min;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Cluster, Edge as GEdge, Node};
    use crate::oracle;
    use std::collections::HashMap;

    fn node(id: &str) -> Node {
        Node::new(id, 72.0, 36.0)
    }

    /// Build a Graph from node ids and (tail, head) edges.
    fn graph(ids: &[&str], edges: &[(&str, &str)]) -> Graph {
        let mut g = Graph::new();
        g.nodes = ids.iter().map(|i| node(i)).collect();
        g.edges = edges.iter().map(|(t, h)| GEdge::new(*t, *h)).collect();
        g
    }

    fn ranks_by_id(g: &Graph) -> HashMap<String, i32> {
        let r = assign_ranks(g);
        g.nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), r.rank[i]))
            .collect()
    }

    #[test]
    fn ranks_match_dot_on_a_diamond() {
        let g = graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let mine = ranks_by_id(&g);
        let Some(o) = oracle::run(&g) else {
            eprintln!("dot not installed — skipping oracle comparison");
            return;
        };
        let theirs = o.ranks();
        for id in ["a", "b", "c", "d"] {
            assert_eq!(
                mine[id],
                i32::try_from(theirs[id]).unwrap(),
                "rank of {id}: mine={} dot={}",
                mine[id],
                theirs[id]
            );
        }
    }

    #[test]
    fn ranks_match_dot_on_a_deeper_graph() {
        // A chain with a skip edge and a fan-out.
        let g = graph(
            &["a", "b", "c", "d", "e"],
            &[
                ("a", "b"),
                ("b", "c"),
                ("c", "d"),
                ("a", "d"),
                ("b", "e"),
                ("e", "d"),
            ],
        );
        let mine = ranks_by_id(&g);
        let Some(o) = oracle::run(&g) else {
            return;
        };
        let theirs = o.ranks();
        for n in &g.nodes {
            assert_eq!(
                mine[&n.id],
                i32::try_from(theirs[&n.id]).unwrap(),
                "rank of {}",
                n.id
            );
        }
    }

    #[test]
    fn cycle_is_broken_and_ranked() {
        let g = graph(&["a", "b", "c"], &[("a", "b"), ("b", "c"), ("c", "a")]);
        let r = assign_ranks(&g);
        assert_eq!(r.reversed.iter().filter(|&&x| x).count(), 1);
        // Three nodes on three distinct ranks after breaking the cycle.
        let mut ranks = r.rank.clone();
        ranks.sort_unstable();
        assert_eq!(ranks, vec![0, 1, 2]);
    }

    #[test]
    fn cluster_members_are_contiguous_and_externals_are_outside() {
        // Caller -> {m1 -> m2 (clustered)} -> Callee. The band must hold m1, m2;
        // Caller strictly above it, Callee strictly below.
        let mut g = graph(
            &["caller", "m1", "m2", "callee"],
            &[("caller", "m1"), ("m1", "m2"), ("m2", "callee")],
        );
        g.clusters.push(Cluster {
            id: "box".to_owned(),
            members: vec!["m1".to_owned(), "m2".to_owned()],
            margin: 8.0,
        });
        let r = ranks_by_id(&g);
        // Band = ranks of m1, m2.
        let (b_lo, b_hi) = (r["m1"].min(r["m2"]), r["m1"].max(r["m2"]));
        assert!(b_hi - b_lo == 1, "two members on adjacent ranks");
        assert!(r["caller"] < b_lo, "caller above the band: {r:?}");
        assert!(r["callee"] > b_hi, "callee below the band: {r:?}");
    }
}
