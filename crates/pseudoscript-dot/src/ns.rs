//! Network simplex — the optimal integer-ranking solver behind both the rank
//! pass (`dot` `rank2(balance=1)`) and the x-coordinate pass (`balance=2`),
//! ported from Graphviz `lib/common/ns.c` (Gansner et al. 1993, §2.3–2.4).
//!
//! Given nodes and a set of difference constraints `rank[head] - rank[tail] >=
//! minlen`, it finds integer ranks minimising `Σ weight · (rank[head] -
//! rank[tail])`. The classic four steps: an initial feasible ranking
//! (`init_rank`), a tight spanning tree (`feasible_tree`), then pivots that
//! swap a negative-cut-value tree edge out for the tightest edge crossing its
//! cut until no negative cut value remains, and finally a balancing pass.
//!
//! This port keeps Graphviz's algorithm and result but not its intrusive data
//! structures: cut values are recomputed from the tree's `low`/`lim` postorder
//! numbering rather than maintained incrementally. The graphs here are small
//! (diagram-sized), so the simpler full recompute is the right trade — fidelity
//! is verified against the real `dot` binary in the rank/position oracle tests.

/// A difference constraint `rank[head] - rank[tail] >= minlen`, weighted.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Constraint {
    pub tail: usize,
    pub head: usize,
    pub minlen: i32,
    pub weight: i32,
}

/// How to settle the remaining freedom after the optimal ranking is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Balance {
    /// Leave ranks as the simplex produced them (only normalised to min 0).
    None,
    /// `dot` `balance=1`: move each loose node to the least-populated feasible
    /// rank — spreads nodes evenly down the ranks.
    TopBottom,
}

/// One working edge (an input constraint, plus artificial edges added to span a
/// disconnected graph).
#[derive(Debug, Clone, Copy)]
struct Edge {
    tail: usize,
    head: usize,
    minlen: i32,
    weight: i32,
    tree: bool,
    cut: i32,
}

/// Solve the ranking problem for `n` nodes under `constraints`, returning a rank
/// per node. Deterministic; never panics. Handles disconnected inputs (each
/// component is spanned via weight-0 artificial edges, which never affect the
/// objective).
pub(crate) fn rank(n: usize, constraints: &[Constraint], balance: Balance) -> Vec<i32> {
    let mut ns = Ns::new(n, constraints);
    if n == 0 {
        return Vec::new();
    }
    ns.init_rank();
    ns.feasible_tree();
    ns.pivot_loop();
    ns.normalize();
    match balance {
        Balance::None => {}
        Balance::TopBottom => ns.tb_balance(),
    }
    ns.normalize();
    ns.rank
}

struct Ns {
    n: usize,
    edges: Vec<Edge>,
    out_adj: Vec<Vec<usize>>,
    in_adj: Vec<Vec<usize>>,
    rank: Vec<i32>,
    // Tree structure (valid after `build_tree_structure`).
    par: Vec<Option<usize>>, // parent tree-edge index, None at the root
    low: Vec<i32>,
    lim: Vec<i32>,
}

impl Ns {
    fn new(n: usize, constraints: &[Constraint]) -> Self {
        let edges: Vec<Edge> = constraints
            .iter()
            .map(|c| Edge {
                tail: c.tail,
                head: c.head,
                minlen: c.minlen,
                weight: c.weight,
                tree: false,
                cut: 0,
            })
            .collect();
        let mut s = Self {
            n,
            edges,
            out_adj: vec![Vec::new(); n],
            in_adj: vec![Vec::new(); n],
            rank: vec![0; n],
            par: vec![None; n],
            low: vec![0; n],
            lim: vec![0; n],
        };
        s.reindex();
        s
    }

    /// Rebuild the per-node adjacency from the edge list.
    fn reindex(&mut self) {
        for v in &mut self.out_adj {
            v.clear();
        }
        for v in &mut self.in_adj {
            v.clear();
        }
        for (i, e) in self.edges.iter().enumerate() {
            self.out_adj[e.tail].push(i);
            self.in_adj[e.head].push(i);
        }
    }

    fn slack(&self, e: usize) -> i32 {
        let ed = &self.edges[e];
        self.rank[ed.head] - self.rank[ed.tail] - ed.minlen
    }

    /// Kahn longest-path: a feasible initial ranking. Requires an acyclic
    /// constraint graph (the caller breaks cycles first).
    fn init_rank(&mut self) {
        let mut indeg: Vec<usize> = (0..self.n).map(|v| self.in_adj[v].len()).collect();
        let mut queue: Vec<usize> = (0..self.n).filter(|&v| indeg[v] == 0).collect();
        let mut head = 0;
        let mut done = 0;
        while head < queue.len() {
            let v = queue[head];
            head += 1;
            done += 1;
            let mut r = 0;
            for &e in &self.in_adj[v] {
                let ed = &self.edges[e];
                r = r.max(self.rank[ed.tail] + ed.minlen);
            }
            self.rank[v] = r;
            for &e in &self.out_adj[v] {
                let h = self.edges[e].head;
                indeg[h] -= 1;
                if indeg[h] == 0 {
                    queue.push(h);
                }
            }
        }
        debug_assert_eq!(done, self.n, "init_rank requires an acyclic graph");
        let _ = done;
    }

    /// Build a tight spanning tree, shifting components to make incident edges
    /// tight, adding weight-0 artificial edges across disconnected components.
    fn feasible_tree(&mut self) {
        loop {
            let (in_tree, count) = self.grow_tight_tree();
            if count == self.n {
                break;
            }
            if let Some(e) = self.min_slack_incident(&in_tree) {
                let mut delta = self.slack(e);
                if in_tree[self.edges[e].head] {
                    delta = -delta;
                }
                for (v, &member) in in_tree.iter().enumerate() {
                    if member {
                        self.rank[v] += delta;
                    }
                }
            } else {
                // Disconnected: tie an unreached node's component to the tree
                // with a weight-0, minlen-0 artificial edge (objective-neutral).
                let Some(u) = (0..self.n).find(|&v| !in_tree[v]) else {
                    break;
                };
                let root = (0..self.n).find(|&v| in_tree[v]).unwrap_or(0);
                let shift = self.rank[root] - self.rank[u];
                let comp = self.component_of(u);
                for v in comp {
                    self.rank[v] += shift;
                }
                self.edges.push(Edge {
                    tail: root,
                    head: u,
                    minlen: 0,
                    weight: 0,
                    tree: false,
                    cut: 0,
                });
                self.reindex();
            }
        }
    }

    /// Grow a maximal tree of tight (slack-0) edges from node 0, marking which
    /// edges are tree edges. Returns the membership mask and node count.
    fn grow_tight_tree(&mut self) -> (Vec<bool>, usize) {
        for e in &mut self.edges {
            e.tree = false;
        }
        let mut in_tree = vec![false; self.n];
        in_tree[0] = true;
        let mut count = 1;
        loop {
            let mut added = false;
            for i in 0..self.edges.len() {
                let (t, h) = (self.edges[i].tail, self.edges[i].head);
                let crosses = in_tree[t] ^ in_tree[h];
                if crosses && self.slack(i) == 0 {
                    self.edges[i].tree = true;
                    in_tree[t] = true;
                    in_tree[h] = true;
                    count += 1;
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        (in_tree, count)
    }

    /// The minimum-slack non-tree edge with exactly one endpoint in the tree.
    fn min_slack_incident(&self, in_tree: &[bool]) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for i in 0..self.edges.len() {
            let (t, h) = (self.edges[i].tail, self.edges[i].head);
            if in_tree[t] ^ in_tree[h] {
                let s = self.slack(i);
                if best.is_none_or(|(bs, _)| s < bs) {
                    best = Some((s, i));
                }
            }
        }
        best.map(|(_, i)| i)
    }

    /// The set of nodes reachable from `start` over real (undirected) edges.
    fn component_of(&self, start: usize) -> Vec<usize> {
        let mut seen = vec![false; self.n];
        let mut stack = vec![start];
        seen[start] = true;
        let mut out = Vec::new();
        while let Some(v) = stack.pop() {
            out.push(v);
            for &e in self.out_adj[v].iter().chain(&self.in_adj[v]) {
                let other = if self.edges[e].tail == v {
                    self.edges[e].head
                } else {
                    self.edges[e].tail
                };
                if !seen[other] {
                    seen[other] = true;
                    stack.push(other);
                }
            }
        }
        out
    }

    /// Root the spanning tree at node 0 and assign `par`, `low`, `lim` (postorder
    /// DFS numbering). `lim` is the postorder index; `low` the smallest `lim` in
    /// the subtree — together they answer "is x in this subtree?" in O(1).
    fn build_tree_structure(&mut self) {
        for v in &mut self.par {
            *v = None;
        }
        let mut lim = 1;
        // Iterative DFS over tree edges from root 0.
        // frame: (node, parent_edge, child cursor over incident tree edges)
        let tree_adj = self.tree_adjacency();
        let mut stack: Vec<(usize, Option<usize>, usize)> = vec![(0, None, 0)];
        self.low[0] = 1;
        while let Some(&mut (v, par_e, ref mut cursor)) = stack.last_mut() {
            if *cursor == 0 {
                self.par[v] = par_e;
                self.low[v] = lim;
            }
            if *cursor < tree_adj[v].len() {
                let e = tree_adj[v][*cursor];
                *cursor += 1;
                if Some(e) == par_e {
                    continue;
                }
                let child = self.other(e, v);
                self.low[child] = lim;
                stack.push((child, Some(e), 0));
            } else {
                self.lim[v] = lim;
                lim += 1;
                stack.pop();
            }
        }
    }

    /// Incident tree edges per node.
    fn tree_adjacency(&self) -> Vec<Vec<usize>> {
        let mut adj = vec![Vec::new(); self.n];
        for (i, e) in self.edges.iter().enumerate() {
            if e.tree {
                adj[e.tail].push(i);
                adj[e.head].push(i);
            }
        }
        adj
    }

    fn other(&self, e: usize, v: usize) -> usize {
        let ed = &self.edges[e];
        if ed.tail == v { ed.head } else { ed.tail }
    }

    /// Whether node `x` lies in the subtree rooted at the deeper endpoint of
    /// tree edge `e` (the child side).
    fn in_child_subtree(&self, e: usize, x: usize) -> bool {
        let child = self.child_of(e);
        self.low[child] <= self.lim[x] && self.lim[x] <= self.lim[child]
    }

    /// The deeper (child) endpoint of tree edge `e`.
    fn child_of(&self, e: usize) -> usize {
        let ed = &self.edges[e];
        if self.par[ed.head] == Some(e) {
            ed.head
        } else {
            ed.tail
        }
    }

    /// Recompute every tree edge's cut value from the component split it induces.
    /// `cut(e) = Σ weight(f) for f crossing tail→head − Σ weight(f) head→tail`,
    /// where the tail side is the component containing `e.tail`.
    fn compute_cutvalues(&mut self) {
        for i in 0..self.edges.len() {
            if !self.edges[i].tree {
                continue;
            }
            let tail_side_is_child = self.edges[i].tail == self.child_of(i);
            let in_tail = |x: usize| self.in_child_subtree(i, x) == tail_side_is_child;
            let mut cut = 0;
            for f in &self.edges {
                let (ft, fh) = (in_tail(f.tail), in_tail(f.head));
                if ft && !fh {
                    cut += f.weight;
                } else if !ft && fh {
                    cut -= f.weight;
                }
            }
            self.edges[i].cut = cut;
        }
    }

    /// The simplex pivots: while a tree edge has a negative cut value, replace it
    /// with the tightest edge crossing its cut, and retighten.
    fn pivot_loop(&mut self) {
        let mut guard = 0;
        let max_iter = (self.n + 1) * (self.edges.len() + 1) * 4 + 16;
        loop {
            self.build_tree_structure();
            self.compute_cutvalues();
            let Some(leave) =
                (0..self.edges.len()).find(|&i| self.edges[i].tree && self.edges[i].cut < 0)
            else {
                break;
            };
            let Some(enter) = self.enter_edge(leave) else {
                break;
            };
            self.edges[leave].tree = false;
            self.edges[enter].tree = true;
            self.retighten();
            guard += 1;
            if guard > max_iter {
                break; // safety: never spin (graphs are tiny; optimum is reached well before)
            }
        }
    }

    /// The entering edge for a leaving tree edge: the minimum-slack non-tree edge
    /// crossing the cut in the opposite orientation (head-component → tail-side).
    fn enter_edge(&self, leave: usize) -> Option<usize> {
        let tail_side_is_child = self.edges[leave].tail == self.child_of(leave);
        let in_tail = |x: usize| self.in_child_subtree(leave, x) == tail_side_is_child;
        let mut best: Option<(i32, usize)> = None;
        for i in 0..self.edges.len() {
            if self.edges[i].tree {
                continue;
            }
            let (t, h) = (self.edges[i].tail, self.edges[i].head);
            // Reconnect: f.tail in head-component, f.head in tail-component.
            if !in_tail(t) && in_tail(h) {
                let s = self.slack(i);
                if best.is_none_or(|(bs, _)| s < bs) {
                    best = Some((s, i));
                }
            }
        }
        best.map(|(_, i)| i)
    }

    /// Recompute ranks so every tree edge is tight, propagating from root 0.
    fn retighten(&mut self) {
        self.build_tree_structure();
        let tree_adj = self.tree_adjacency();
        let mut visited = vec![false; self.n];
        let mut stack = vec![0];
        visited[0] = true;
        while let Some(v) = stack.pop() {
            for &e in &tree_adj[v] {
                let child = self.other(e, v);
                if visited[child] {
                    continue;
                }
                visited[child] = true;
                let ed = &self.edges[e];
                self.rank[child] = if ed.tail == v {
                    self.rank[v] + ed.minlen // v -> child
                } else {
                    self.rank[v] - ed.minlen // child -> v
                };
                stack.push(child);
            }
        }
    }

    /// Shift so the minimum rank is 0.
    fn normalize(&mut self) {
        if let Some(&min) = self.rank.iter().min() {
            for r in &mut self.rank {
                *r -= min;
            }
        }
    }

    /// `dot` `TB_balance`: move each node whose in- and out-weights are equal to
    /// the least-populated rank within its feasible `[low, high]` window.
    fn tb_balance(&mut self) {
        let max_rank = self.rank.iter().copied().max().unwrap_or(0);
        if max_rank < 0 {
            return;
        }
        let idx = |r: i32| usize::try_from(r).unwrap_or(0);
        let mut nrank = vec![0i32; idx(max_rank) + 1];
        for &r in &self.rank {
            nrank[idx(r)] += 1;
        }
        // Visit in increasing rank for determinism.
        let mut order: Vec<usize> = (0..self.n).collect();
        order.sort_by_key(|&v| (self.rank[v], v));
        for v in order {
            let mut inw = 0;
            let mut outw = 0;
            let mut low = 0;
            let mut high = max_rank;
            for &e in &self.in_adj[v] {
                let ed = &self.edges[e];
                inw += ed.weight;
                low = low.max(self.rank[ed.tail] + ed.minlen);
            }
            for &e in &self.out_adj[v] {
                let ed = &self.edges[e];
                outw += ed.weight;
                high = high.min(self.rank[ed.head] - ed.minlen);
            }
            if inw == outw && low <= high {
                let mut choice = low;
                let mut c = low + 1;
                while c <= high {
                    if nrank[idx(c)] < nrank[idx(choice)] {
                        choice = c;
                    }
                    c += 1;
                }
                nrank[idx(self.rank[v])] -= 1;
                nrank[idx(choice)] += 1;
                self.rank[v] = choice;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(tail: usize, head: usize) -> Constraint {
        Constraint {
            tail,
            head,
            minlen: 1,
            weight: 1,
        }
    }

    #[test]
    fn chain_ranks_increase_by_one() {
        // 0 -> 1 -> 2 -> 3
        let cs = [c(0, 1), c(1, 2), c(2, 3)];
        let r = rank(4, &cs, Balance::None);
        assert_eq!(r, vec![0, 1, 2, 3]);
    }

    #[test]
    fn diamond_ranks() {
        // 0 -> {1,2} -> 3
        let cs = [c(0, 1), c(0, 2), c(1, 3), c(2, 3)];
        let r = rank(4, &cs, Balance::None);
        assert_eq!(r[0], 0);
        assert_eq!(r[1], 1);
        assert_eq!(r[2], 1);
        assert_eq!(r[3], 2);
    }

    #[test]
    fn long_edge_is_pulled_tight() {
        // 0->1->2 and 0->2: network simplex keeps 2 at rank 2 (minimises length).
        let cs = [c(0, 1), c(1, 2), c(0, 2)];
        let r = rank(3, &cs, Balance::None);
        assert_eq!(r, vec![0, 1, 2]);
    }

    #[test]
    fn disconnected_components_each_normalize() {
        // Two independent chains; both start at rank 0.
        let cs = [c(0, 1), c(2, 3)];
        let r = rank(4, &cs, Balance::None);
        assert_eq!(r[0], 0);
        assert_eq!(r[1], 1);
        assert_eq!(r[2], 0);
        assert_eq!(r[3], 1);
    }

    #[test]
    fn minlen_respected() {
        let cs = [Constraint {
            tail: 0,
            head: 1,
            minlen: 3,
            weight: 1,
        }];
        let r = rank(2, &cs, Balance::None);
        assert_eq!(r, vec![0, 3]);
    }

    #[test]
    fn deterministic() {
        let cs = [c(0, 1), c(0, 2), c(1, 3), c(2, 3), c(0, 3)];
        let a = rank(4, &cs, Balance::None);
        let b = rank(4, &cs, Balance::None);
        assert_eq!(a, b);
    }
}
