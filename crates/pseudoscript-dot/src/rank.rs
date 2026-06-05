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

use crate::cluster::ClusterTree;
use crate::graph::Graph;
use crate::ns::{Balance, Constraint, rank};

/// Upper bound on an edge's `minlen` (rank span). A diagram never needs a single
/// edge to span more ranks than this; clamping keeps the total rank count — and
/// thus the engine's memory and time — bounded on adversarial input, so the
/// wasm-safe "never panics, never hangs" contract holds even for absurd inputs.
const MAX_MINLEN: i32 = 1024;

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
            minlen: graph.edges[i].minlen.clamp(1, MAX_MINLEN),
            weight: graph.edges[i].weight.max(1),
        });
    }

    let mut rank_vec = if graph.clusters.is_empty() {
        let cs: Vec<Constraint> = eff.iter().map(Edge::constraint).collect();
        rank(n, &cs, Balance::TopBottom)
    } else {
        let tree = ClusterTree::build(graph, n);
        rank_with_clusters(n, &eff, &tree)
    };

    apply_same_rank(graph, &mut rank_vec);

    Ranking {
        rank: rank_vec,
        reversed,
    }
}

/// Pull each same-rank group onto one rank — the shallowest (smallest) member
/// rank — then renormalise. Applied as a post-pass over final ranks, so it works
/// across cluster boundaries; the forcing edges it makes point backward are
/// handled at draw time (direction is read from final ranks, [`crate::mincross`]).
fn apply_same_rank(graph: &Graph, rank: &mut [i32]) {
    for group in &graph.same_rank {
        let members: Vec<usize> = group.iter().filter_map(|id| graph.node_index(id)).collect();
        let Some(target) = members.iter().map(|&v| rank[v]).min() else {
            continue;
        };
        for v in members {
            rank[v] = target;
        }
    }
    if let Some(&min) = rank.iter().min() {
        for r in rank.iter_mut() {
            *r -= min;
        }
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

/// An atom in a cluster's (or the root's) local ranking problem: either a node
/// placed directly at this level, or a nested child cluster (represented by a
/// top/bottom skeleton pair). Mirrors `dot`'s collapse of a `subgraph cluster_*`
/// to a skeleton in its parent (`lib/dotgen/cluster.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Atom {
    Node(usize),
    Child(usize),
}

/// Rank a graph with nested clusters by recursive collapse/expand, faithful to
/// `dot`'s `dot_rank`/`collapse_cluster`/`expand_cluster`. Each cluster is ranked
/// on its own local problem (direct members plus each child collapsed to a
/// skeleton band), then collapsed into its parent; the root graph is ranked over
/// its free nodes and top-level clusters; finally absolute ranks are assigned
/// top-down so every cluster's band is a contiguous (and properly nested)
/// sub-band.
fn rank_with_clusters(n: usize, eff: &[Edge], tree: &ClusterTree) -> Vec<i32> {
    let nc = tree.parent.len();
    let mut member_local_rank = vec![0i32; n]; // a node's rank within its owning cluster
    let mut child_local_top = vec![0i32; nc]; // a cluster's top within its parent's coords
    let mut height = vec![0i32; nc]; // a cluster's internal band height

    // 1. Bottom-up: solve each cluster's local problem (children first, so their
    //    heights are known when their parent collapses them).
    for &ci in &tree.post_order {
        let members: Vec<usize> = (0..n).filter(|&v| tree.owner[v] == Some(ci)).collect();
        let children = &tree.children[ci];
        let solved = solve_level(&members, children, &height, eff, |v| {
            atom_within(tree, ci, v)
        });
        for (k, &v) in members.iter().enumerate() {
            member_local_rank[v] = solved.member_ranks[k];
        }
        for (j, &child) in children.iter().enumerate() {
            child_local_top[child] = solved.child_tops[j];
        }
        height[ci] = solved.height;
    }

    // 2. Root problem: free nodes plus the top-level clusters.
    let free: Vec<usize> = (0..n).filter(|&v| tree.owner[v].is_none()).collect();
    let root = solve_level(&free, &tree.roots, &height, eff, |v| {
        Some(atom_at_root(tree, v))
    });
    let mut free_rank = vec![0i32; n];
    for (k, &v) in free.iter().enumerate() {
        free_rank[v] = root.member_ranks[k];
    }
    let mut root_top = vec![0i32; nc];
    for (j, &ci) in tree.roots.iter().enumerate() {
        root_top[ci] = root.child_tops[j];
    }

    // 3. Expand top-down into absolute ranks.
    let mut out = vec![0i32; n];
    for (v, slot) in out.iter_mut().enumerate() {
        if tree.owner[v].is_none() {
            *slot = free_rank[v];
        }
    }
    for &ci in &tree.roots {
        assign_absolute(
            tree,
            ci,
            root_top[ci],
            &member_local_rank,
            &child_local_top,
            &mut out,
        );
    }
    if let Some(&min) = out.iter().min() {
        for r in &mut out {
            *r -= min;
        }
    }
    out
}

/// The atom a node maps to within cluster `ci`: itself when directly owned by
/// `ci`, the child cluster of `ci` it descends through otherwise, or `None` when
/// the node lies outside `ci` (its edge is handled at an ancestor level).
fn atom_within(tree: &ClusterTree, ci: usize, v: usize) -> Option<Atom> {
    let chain = tree.ancestry(v); // [owner, parent(owner), …]
    match chain.iter().position(|&c| c == ci) {
        None => None,
        Some(0) => Some(Atom::Node(v)),
        Some(k) => Some(Atom::Child(chain[k - 1])),
    }
}

/// The atom a node maps to at the root level: a free node is itself, a clustered
/// node is its outermost (top-level) ancestor cluster.
fn atom_at_root(tree: &ClusterTree, v: usize) -> Atom {
    match tree.ancestry(v).last() {
        Some(&root) => Atom::Child(root),
        None => Atom::Node(v),
    }
}

/// The ranks a [`solve_level`] call assigns, parallel to its member/child inputs.
struct LevelRanks {
    /// Local rank per member node, parallel to `members`.
    member_ranks: Vec<i32>,
    /// Local top rank per child cluster, parallel to `children`.
    child_tops: Vec<i32>,
    /// The level's band height (its deepest local rank).
    height: i32,
}

/// Solve one level's local ranking problem: `members` are nodes placed directly
/// here; each child in `children` collapses to a top/bottom skeleton pair spanning
/// its own band (`height[child]`). `atom_of` maps a graph node to its atom at this
/// level (or `None` to ignore an out-of-level endpoint). Returns local ranks with
/// the level's top at 0.
fn solve_level(
    members: &[usize],
    children: &[usize],
    height: &[i32],
    eff: &[Edge],
    atom_of: impl Fn(usize) -> Option<Atom>,
) -> LevelRanks {
    let member_idx: std::collections::HashMap<usize, usize> =
        members.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    let child_idx: std::collections::HashMap<usize, usize> =
        children.iter().enumerate().map(|(j, &c)| (c, j)).collect();
    let base = members.len();
    let top = |j: usize| base + 2 * j; // child j's top skeleton node
    let bottom = |j: usize| base + 2 * j + 1; // child j's bottom skeleton node
    let local_n = base + 2 * children.len();

    // An edge tail leaves a child from its band bottom; an edge head enters at the
    // band top — so the skeleton spans the child's whole band.
    let leave = |a: Atom| match a {
        Atom::Node(v) => member_idx.get(&v).copied(),
        Atom::Child(c) => child_idx.get(&c).map(|&j| bottom(j)),
    };
    let enter = |a: Atom| match a {
        Atom::Node(v) => member_idx.get(&v).copied(),
        Atom::Child(c) => child_idx.get(&c).map(|&j| top(j)),
    };

    let mut cs: Vec<Constraint> = Vec::new();
    for e in eff {
        let (Some(a), Some(b)) = (atom_of(e.tail), atom_of(e.head)) else {
            continue;
        };
        if a == b {
            continue; // internal to a child (or a self-atom): handled deeper
        }
        if let (Some(tail), Some(head)) = (leave(a), enter(b)) {
            cs.push(Constraint {
                tail,
                head,
                minlen: e.minlen,
                weight: e.weight,
            });
        }
    }
    // Band-height edges hold each child skeleton's top above its bottom by the
    // child's internal height (clamped, so deep nesting can't compound spans into
    // an unbounded rank count).
    for (j, &child) in children.iter().enumerate() {
        cs.push(Constraint {
            tail: top(j),
            head: bottom(j),
            minlen: height[child].clamp(0, MAX_MINLEN),
            weight: 1,
        });
    }

    let solved = rank(local_n, &cs, Balance::None);
    LevelRanks {
        member_ranks: (0..members.len()).map(|i| solved[i]).collect(),
        child_tops: (0..children.len()).map(|j| solved[top(j)]).collect(),
        height: solved.iter().copied().max().unwrap_or(0),
    }
}

/// Assign absolute ranks within cluster `ci`, whose band starts at `abs_top`:
/// each direct member at `abs_top + its local rank`, then recurse into each child
/// at `abs_top + the child's local top`.
fn assign_absolute(
    tree: &ClusterTree,
    ci: usize,
    abs_top: i32,
    member_local_rank: &[i32],
    child_local_top: &[i32],
    out: &mut [i32],
) {
    for (v, slot) in out.iter_mut().enumerate() {
        if tree.owner[v] == Some(ci) {
            *slot = abs_top + member_local_rank[v];
        }
    }
    for &child in &tree.children[ci] {
        assign_absolute(
            tree,
            child,
            abs_top + child_local_top[child],
            member_local_rank,
            child_local_top,
            out,
        );
    }
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

    /// outer `O` ⊇ inner `I {m1,m2}` + sibling `sib`; `top` above, `bot` below.
    fn nested_graph() -> Graph {
        let mut g = graph(
            &["top", "m1", "m2", "sib", "bot"],
            &[
                ("top", "m1"),
                ("m1", "m2"),
                ("m2", "bot"),
                ("top", "sib"),
                ("sib", "bot"),
            ],
        );
        g.clusters.push(Cluster {
            id: "O".to_owned(),
            parent: None,
            members: vec!["sib".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        g.clusters.push(Cluster {
            id: "I".to_owned(),
            parent: Some("O".to_owned()),
            members: vec!["m1".to_owned(), "m2".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        g
    }

    #[test]
    fn nested_inner_band_is_contiguous_sub_band_of_outer() {
        let g = nested_graph();
        let r = ranks_by_id(&g);
        // Inner band {m1,m2} on adjacent ranks.
        assert_eq!(
            (r["m1"] - r["m2"]).abs(),
            1,
            "inner members adjacent: {r:?}"
        );
        // Outer band spans all of {m1,m2,sib}; inner band sits within it.
        let outer_lo = r["m1"].min(r["m2"]).min(r["sib"]);
        let outer_hi = r["m1"].max(r["m2"]).max(r["sib"]);
        let inner_lo = r["m1"].min(r["m2"]);
        let inner_hi = r["m1"].max(r["m2"]);
        assert!(
            inner_lo >= outer_lo && inner_hi <= outer_hi,
            "inner ⊆ outer: {r:?}"
        );
        // Externals strictly outside the outer band.
        assert!(r["top"] < outer_lo, "top above outer band: {r:?}");
        assert!(r["bot"] > outer_hi, "bot below outer band: {r:?}");
    }

    #[test]
    fn nested_cluster_ranks_match_dot() {
        let g = nested_graph();
        let mine = ranks_by_id(&g);
        let Some(o) = oracle::run(&g) else {
            eprintln!("dot not installed — skipping oracle comparison");
            return;
        };
        let theirs = o.ranks();
        for n in &g.nodes {
            assert_eq!(
                mine[&n.id],
                i32::try_from(theirs[&n.id]).unwrap(),
                "rank of {} (mine={} dot={})",
                n.id,
                mine[&n.id],
                theirs[&n.id]
            );
        }
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
            parent: None,
            members: vec!["m1".to_owned(), "m2".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        let r = ranks_by_id(&g);
        // Band = ranks of m1, m2.
        let (b_lo, b_hi) = (r["m1"].min(r["m2"]), r["m1"].max(r["m2"]));
        assert!(b_hi - b_lo == 1, "two members on adjacent ranks");
        assert!(r["caller"] < b_lo, "caller above the band: {r:?}");
        assert!(r["callee"] > b_hi, "callee below the band: {r:?}");
    }
}
