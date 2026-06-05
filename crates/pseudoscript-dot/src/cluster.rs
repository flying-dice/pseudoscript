//! The cluster tree derived from a [`Graph`]'s flat cluster list — `dot`'s
//! `GD_clust`/`ND_clust`/`GD_parent` structure (`lib/dotgen/cluster.c`).
//!
//! A [`crate::graph::Cluster`] names its enclosing cluster by id; this module
//! resolves those ids into indices, builds the parent/child tree, and records
//! each node's innermost owning cluster. It is **total and non-panicking**: a
//! cluster whose `parent` is missing or forms a cycle is treated as a root, and
//! a node listed by more than one cluster is owned by the first (the rest drop
//! it), mirroring `dot`'s `node_induce` which discards doubly-claimed nodes.

use crate::graph::Graph;

/// The resolved cluster tree for a graph.
#[derive(Debug, Clone)]
pub(crate) struct ClusterTree {
    /// Parent cluster index per cluster, or `None` for a root cluster.
    pub parent: Vec<Option<usize>>,
    /// Child cluster indices per cluster, in input order.
    pub children: Vec<Vec<usize>>,
    /// Root clusters (no parent), in input order.
    pub roots: Vec<usize>,
    /// Clusters ordered children-before-parents (leaves first) — the order to
    /// rank/position bottom-up.
    pub post_order: Vec<usize>,
    /// Innermost owning cluster per graph node (index into `graph.nodes`), or
    /// `None` when the node is free (in no cluster).
    pub owner: Vec<Option<usize>>,
}

impl ClusterTree {
    /// Resolve `graph`'s flat cluster list into a tree. `n` is the node count.
    pub(crate) fn build(graph: &Graph, n: usize) -> Self {
        let nc = graph.clusters.len();

        // Resolve each cluster's parent id to an index.
        let index_of: std::collections::HashMap<&str, usize> = graph
            .clusters
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.as_str(), i))
            .collect();
        let mut parent: Vec<Option<usize>> = graph
            .clusters
            .iter()
            .map(|c| c.parent.as_deref().and_then(|p| index_of.get(p).copied()))
            .collect();
        // Break any parent cycle by demoting a cluster on the cycle to a root.
        for ci in 0..nc {
            if Self::reaches_self(&parent, ci) {
                parent[ci] = None;
            }
        }

        let mut children = vec![Vec::new(); nc];
        let mut roots = Vec::new();
        for (ci, p) in parent.iter().enumerate() {
            match p {
                Some(pi) => children[*pi].push(ci),
                None => roots.push(ci),
            }
        }

        let post_order = Self::post_order(&children, &roots);

        // Owner: the first cluster whose `members` lists the node wins; a later
        // claim on an already-owned node is dropped.
        let mut owner = vec![None; n];
        for (ci, c) in graph.clusters.iter().enumerate() {
            for m in &c.members {
                if let Some(v) = graph.node_index(m)
                    && owner[v].is_none()
                {
                    owner[v] = Some(ci);
                }
            }
        }

        Self {
            parent,
            children,
            roots,
            post_order,
            owner,
        }
    }

    /// Whether following `parent` from `start` returns to `start` — a cycle.
    fn reaches_self(parent: &[Option<usize>], start: usize) -> bool {
        let mut cur = parent[start];
        let mut steps = 0;
        while let Some(c) = cur {
            if c == start {
                return true;
            }
            if steps > parent.len() {
                return true; // cycle not through `start`, still unbounded
            }
            cur = parent[c];
            steps += 1;
        }
        false
    }

    /// Clusters in children-before-parents order via DFS from the roots.
    fn post_order(children: &[Vec<usize>], roots: &[usize]) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack: Vec<(usize, bool)> = roots.iter().rev().map(|&r| (r, false)).collect();
        while let Some((ci, expanded)) = stack.pop() {
            if expanded {
                out.push(ci);
            } else {
                stack.push((ci, true));
                for &child in children[ci].iter().rev() {
                    stack.push((child, false));
                }
            }
        }
        out
    }

    /// A cluster and its ancestors, innermost first (the cluster, its parent, up
    /// to a root). Terminates: the `parent` array is cycle-free after `build`.
    pub(crate) fn cluster_ancestry(&self, ci: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut cur = Some(ci);
        while let Some(c) = cur {
            chain.push(c);
            cur = self.parent[c];
        }
        chain
    }

    /// The owning cluster and its ancestors, innermost first, for a node — empty
    /// when the node is free.
    pub(crate) fn ancestry(&self, node: usize) -> Vec<usize> {
        self.owner[node]
            .map(|ci| self.cluster_ancestry(ci))
            .unwrap_or_default()
    }

    /// The deepest cluster containing both nodes (their lowest common ancestor in
    /// the cluster tree), or `None` when they share no cluster.
    pub(crate) fn common(&self, a: usize, b: usize) -> Option<usize> {
        let chain_b = self.ancestry(b);
        self.ancestry(a).into_iter().find(|c| chain_b.contains(c))
    }
}
