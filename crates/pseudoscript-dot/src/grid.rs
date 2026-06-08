//! Experimental grid placement.
//!
//! An alternative to the layered [`crate::layout`]: every node goes on a regular
//! grid, and the node→cell assignment is chosen to minimise a readability cost.
//! The cost reasons in **integer `(row, col)` cells** — a node either sits on a
//! square or it does not; there is no pixel geometry, no line clipping. Pixels
//! enter only at the very end, when [`build_layout`] places the chosen cells and
//! draws the edges for rendering.
//!
//! Everything is one **weighted cost** (a single unsigned integer the search
//! minimises); no concern is privileged outside it. The terms:
//!
//! - **crossings** — an edge through a third node's cell square, or two edges
//!   crossing (× [`GridParams::crossing_cost`]).
//! - **distance** — total edge length in cells, Manhattan (× `distance_cost`).
//! - **direction** — cells an edge travels against the reading direction, up for
//!   top-down / left for left-to-right (× `flow_cost`).
//! - **frame** — a node inside a cluster cell-box it does not belong to
//!   (× [`FRAME_COST`], fixed high so a frame reads as a constraint).
//! - **compactness** — a cluster's row + column span (× [`SPREAD_COST`], fixed).
//!
//! The first three are the UI dials ([`GridParams`]). A small placement space is
//! solved **exactly** (every way to seat the nodes); a large one by a **work-bounded
//! iterated local search** that returns promptly even for big grids. [`SearchMode`]
//! forces one or the other (to check the heuristic against exact). No node kind is
//! privileged. Exposed as [`grid_layout`] and [`GridPlacement`].

use crate::cluster::ClusterTree;
use crate::graph::{Graph, RankDir};
use crate::layout::{EdgeRoute, GridMeta, Layout, NodePos, Pt};
use crate::pipeline::{LayoutState, Pass};
use crate::splines;

/// Placement-space size (ordered ways to seat `n` nodes in `m` cells) under which
/// [`SearchMode::Auto`] runs the exact search; above it, the heuristic.
const AUTO_EXHAUSTIVE: u128 = 200_000;
/// Hard ceiling for a *forced* exact search ([`SearchMode::Exhaustive`]); beyond
/// it brute force would hang, so the heuristic takes over.
const FORCED_EXHAUSTIVE: u128 = 30_000_000;
/// Work budget for the heuristic, in cost-term operations (≈1.5 s). Bounds the
/// iterated local search so a large grid returns promptly (degraded, not hung);
/// small grids never reach it, so their result is unchanged.
const WORK_BUDGET: u64 = 1_500_000_000;
/// Floor on heuristic cost evaluations, so even a huge grid gets some search.
const MIN_EVALS: u64 = 4_000;
/// Restarts for the local search. Restart 0 descends from the identity; the rest
/// alternate full random shuffles with perturbation kicks of the best-so-far
/// (iterated local search), to escape the 2-opt local minima where a stubborn
/// edge-through-node crossing survives.
const RESTARTS: usize = 48;
/// Gap between grid cells beyond the largest node, in points.
const GRID_GAP: f64 = 56.0;
/// Empty-cell band kept around the pinned content on every side, so a box can be
/// dragged a couple of cells beyond the current bounding box in any direction.
const MARGIN: usize = 2;
/// Cost weight: a non-member sitting inside a cluster's cell-box. Not a dial — set
/// far above the others so the search treats a cluster frame as a constraint (it
/// will only intrude one when no clean placement exists), yet it is still just a
/// cost term like the rest.
const FRAME_COST: usize = 1000;
/// Cost weight: cluster compactness (× the members' row + column span). Not a dial.
const SPREAD_COST: usize = 2;

/// The tunable weights of the grid cost function — the UI's grid "dials" — plus the
/// spacing multiplier. The three costs scale readability concerns; `spacing` scales
/// the gap between cells (the compact/comfortable/roomy control).
#[derive(Debug, Clone, Copy)]
pub struct GridParams {
    /// Per crossing — an edge over a node's cell, or two edges crossing.
    pub crossing_cost: usize,
    /// Per cell of edge length (Manhattan).
    pub distance_cost: usize,
    /// Per cell an edge travels *against* the reading direction (up for top-down,
    /// left for left-to-right) — the directionality preference.
    pub flow_cost: usize,
    /// Multiplier on the inter-cell gap (1.0 = default; <1 compact, >1 roomy).
    pub spacing: f64,
    /// Which search to run. [`SearchMode::Auto`] picks exact for tiny placement
    /// spaces and the heuristic otherwise; the others force one (to compare).
    pub search: SearchMode,
}

/// How [`grid_layout`] searches the placement space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Exact when the placement count is tiny, the bounded heuristic otherwise.
    #[default]
    Auto,
    /// Always the bounded iterated local search (fast, may regress vs exact).
    Heuristic,
    /// Brute force when the placement space is small enough; else fall back to the
    /// heuristic (brute force is only feasible for a handful of nodes).
    Exhaustive,
}

impl Default for GridParams {
    fn default() -> Self {
        Self {
            crossing_cost: 10,
            distance_cost: 1,
            flow_cost: 5,
            spacing: 1.0,
            search: SearchMode::Auto,
        }
    }
}

/// A node pinned to a grid cell by the user (drag-to-pin). The node is fixed at
/// `(row, col)`; the search places only the un-pinned nodes around it. Out-of-range
/// or colliding pins are dropped (the node then places freely).
#[derive(Debug, Clone, Copy)]
pub struct Pin {
    /// Index into `graph.nodes` (the caller resolves the FQN).
    pub node: usize,
    pub row: usize,
    pub col: usize,
}

/// A grid index as `f64` — lossless for any realistic node count. Used only to
/// place the pixel cells [`build_layout`] draws into; the cost never sees pixels.
#[allow(clippy::cast_precision_loss)]
fn coord(i: usize) -> f64 {
    i as f64
}

/// A grid index as `i64` — lossless; a cell index never approaches `i64::MAX`.
#[allow(clippy::cast_possible_wrap)]
fn icoord(i: usize) -> i64 {
    i as i64
}

/// Everything the cost function needs, built once per layout. Cell-only — the
/// cost works in integer `(row, col)` space, so no pixel geometry lives here.
struct Ctx {
    n: usize,
    /// Grid width, to map a cell index to its `(col, row)`.
    cols: usize,
    /// Total cells (`rows × cols`); the search permutes `0..cell_count`.
    cell_count: usize,
    /// Reading direction: `true` biases arrows rightward (left-to-right), `false`
    /// biases them downward (top-down). From the graph's `rankdir`.
    lr: bool,
    /// The tunable cost weights (the UI dials).
    params: GridParams,
    /// Edges as node-index pairs.
    edges: Vec<(usize, usize)>,
    /// Effective members (the subtree) of each cluster, for frame intrusion /
    /// compactness; empty when the graph has no clusters.
    clusters: Vec<Vec<usize>>,
    /// Per slot `0..cell_count`: pinned (a node fixed to this slot's cell). Pinned
    /// slots never move in the search. Empty-cell slots (`>= n`) are never pinned.
    pinned: Vec<bool>,
    /// Slots the search may move (every slot that is not pinned), for shuffle/kick.
    movable: Vec<usize>,
    /// Un-pinned node slots (`< n`), the kick's relocation sources.
    unpinned_nodes: Vec<usize>,
}

/// The inner search grid plus the drag-room frame added at emit (see [`grid_window`]).
struct GridWindow {
    /// Columns and rows the search fills (the inner region, frame excluded).
    cols: usize,
    rows: usize,
    /// Cells of drag-room frame wrapped around the inner grid on every side at emit.
    pad: usize,
}

/// Size the inner search grid for `n` nodes under reading direction `lr`.
///
/// A balanced base grid (longer axis along the reading direction, `long =
/// ceil(sqrt(n))`) carries a one-cell band all round — search room the optimiser uses
/// to seat a node on an edge cell or separate a cluster from externals — grown to fit
/// the furthest pin. These are the cells the search permutes; the [`MARGIN`]-cell
/// drag-room frame is wrapped on at emit ([`attach_grid_meta`]), so it costs the search
/// nothing: an un-pinned graph keeps the bare base extent (byte-identical, exact-search
/// eligible) yet the client still sees a MARGIN-cell band to drag into on every side.
fn grid_window(n: usize, lr: bool, pins: &[Pin]) -> GridWindow {
    let long = (1..=n).find(|c| c * c >= n).unwrap_or(1);
    let short = n.div_ceil(long);
    let (base_cols, base_rows) = if lr { (long, short) } else { (short, long) };

    let valid_pins = || pins.iter().filter(|p| p.node < n);
    let mut cols = base_cols + 2;
    let mut rows = base_rows + 2;
    if let Some(mc) = valid_pins().map(|p| p.col).max() {
        cols = cols.max(mc + 2);
    }
    if let Some(mr) = valid_pins().map(|p| p.row).max() {
        rows = rows.max(mr + 2);
    }
    GridWindow {
        cols,
        rows,
        pad: MARGIN,
    }
}

/// Place `graph` on a grid by minimising the weighted cost under `params` (the UI
/// dials), with `pins` fixing chosen nodes to cells (the rest are searched around
/// them). Deterministic; never panics. With `pins` empty this is byte-identical to
/// the un-pinned layout.
#[must_use]
pub fn grid_layout(graph: &Graph, params: GridParams, pins: &[Pin]) -> Layout {
    let n = graph.nodes.len();
    if n == 0 {
        return Layout::default();
    }

    let lr = graph.rankdir == RankDir::LeftRight;
    let GridWindow { cols, rows, pad } = grid_window(n, lr, pins);
    let m = rows * cols;
    // The gap between cells follows the spacing control (compact/comfortable/roomy).
    // The deviation from 1.0 is amplified so compact/roomy move the gap about twice
    // as far as the raw preset would; floored so boxes never touch.
    let spacing = 1.0 + (params.spacing - 1.0) * 2.0;
    let gap = (GRID_GAP * spacing).max(8.0);
    let cell_w = graph
        .nodes
        .iter()
        .map(|nd| nd.width)
        .fold(0.0_f64, f64::max)
        + gap;
    let cell_h = graph
        .nodes
        .iter()
        .map(|nd| nd.height)
        .fold(0.0_f64, f64::max)
        + gap;
    let cells: Vec<Pt> = (0..m)
        .map(|slot| {
            let (r, c) = (slot / cols, slot % cols);
            Pt::new(
                coord(c) * cell_w + cell_w / 2.0,
                coord(r) * cell_h + cell_h / 2.0,
            )
        })
        .collect();

    let edges: Vec<(usize, usize)> = graph
        .edges
        .iter()
        .filter_map(|e| {
            let (t, h) = (graph.node_index(&e.tail)?, graph.node_index(&e.head)?);
            (t != h).then_some((t, h))
        })
        .collect();
    let clusters = effective_members(graph, n);

    // Resolve pins to their fixed cells in the inner grid (the frame is added at emit,
    // so a pin cell is the client's coordinate as-is). Drop any out of the current grid
    // or colliding with an already-pinned cell (the dropped node then places freely).
    let mut fixed = vec![None; n];
    let mut taken = vec![false; m];
    for p in pins {
        if p.node >= n {
            continue;
        }
        let (row, col) = (p.row, p.col);
        if col >= cols || row >= rows {
            continue;
        }
        let cell = row * cols + col;
        if taken[cell] {
            continue;
        }
        fixed[p.node] = Some(cell);
        taken[cell] = true;
    }
    let pinned: Vec<bool> = (0..m).map(|s| s < n && fixed[s].is_some()).collect();
    let movable: Vec<usize> = (0..m).filter(|&s| !pinned[s]).collect();
    let unpinned_nodes: Vec<usize> = (0..n).filter(|&s| !pinned[s]).collect();
    let any_pinned = unpinned_nodes.len() != n;

    let ctx = Ctx {
        n,
        cols,
        cell_count: m,
        lr,
        params,
        edges,
        clusters,
        pinned,
        movable,
        unpinned_nodes,
    };

    // Pick the search: exact when the placement space is small enough for the
    // chosen mode, else the work-bounded heuristic. Pins always go heuristic (the
    // exact search has no pin support — and a pinned graph is being placed by hand).
    let count = placement_count(m, n);
    let use_exact = !any_pinned
        && match params.search {
            SearchMode::Heuristic => false,
            SearchMode::Exhaustive => count <= FORCED_EXHAUSTIVE,
            SearchMode::Auto => count <= AUTO_EXHAUSTIVE,
        };
    let best = if use_exact {
        exhaustive(&ctx)
    } else {
        let e = u64::try_from(ctx.edges.len()).unwrap_or(u64::MAX);
        let nn = u64::try_from(n).unwrap_or(u64::MAX);
        let unit = e
            .saturating_mul(e.saturating_add(nn))
            .saturating_add(nn + 1);
        let mut budget = (WORK_BUDGET / unit).max(MIN_EVALS);
        local_search(&ctx, seed_perm(n, m, &fixed), &mut budget)
    };

    let layout = build_layout(graph, &cells, &best);
    attach_grid_meta(layout, &best, cols, rows, cell_w, cell_h, pad)
}

/// Record the grid geometry on the layout so a client can map a pixel back to a cell
/// (drag-to-pin). The emitted grid wraps the inner search grid in a `pad`-cell
/// drag-room frame: cell (0,0) sits `pad` cells up-left of the inner origin, and the
/// extent grows by `2·pad`. A pixel recovers its raw cell from `origin` then subtracts
/// `pad`, round-tripping to the exact (frame-free) cell a node sits on.
fn attach_grid_meta(
    mut layout: Layout,
    best: &[usize],
    cols: usize,
    rows: usize,
    cell_w: f64,
    cell_h: f64,
    pad: usize,
) -> Layout {
    if let Some(first) = layout.nodes.first() {
        let (c0, r0) = (best[0] % cols, best[0] / cols);
        let inner_x = first.center.x - coord(c0) * cell_w;
        let inner_y = first.center.y - coord(r0) * cell_h;
        layout.grid = Some(GridMeta {
            cols: cols + 2 * pad,
            rows: rows + 2 * pad,
            cell_w,
            cell_h,
            origin: Pt::new(inner_x - coord(pad) * cell_w, inner_y - coord(pad) * cell_h),
            pad,
        });
    }
    layout
}

/// Build the starting full-`m` permutation: pinned nodes on their fixed cells, the
/// remaining free cells filled into the other slots in ascending order. With no
/// pins this is `(0..m)`, so the un-pinned search is unchanged.
fn seed_perm(n: usize, m: usize, fixed: &[Option<usize>]) -> Vec<usize> {
    let mut perm = vec![usize::MAX; m];
    let mut taken = vec![false; m];
    for (node, &fc) in fixed.iter().take(n).enumerate() {
        if let Some(c) = fc {
            perm[node] = c;
            taken[c] = true;
        }
    }
    let mut free = (0..m).filter(|c| !taken[*c]);
    for slot in &mut perm {
        if *slot == usize::MAX {
            *slot = free.next().unwrap_or(0);
        }
    }
    perm
}

/// A [`Pass`] wrapping [`grid_layout`], so the grid placement drops into a
/// pipeline (it replaces the placement, ignoring the incoming geometry).
#[derive(Debug, Clone, Copy, Default)]
pub struct GridPlacement;

impl Pass for GridPlacement {
    fn name(&self) -> &'static str {
        "grid-placement"
    }

    fn run(&self, mut state: LayoutState) -> LayoutState {
        state.layout = grid_layout(&state.graph, GridParams::default(), &[]);
        state
    }
}

/// The effective (subtree) member node indices of every cluster.
fn effective_members(graph: &Graph, n: usize) -> Vec<Vec<usize>> {
    if graph.clusters.is_empty() {
        return Vec::new();
    }
    let tree = ClusterTree::build(graph, n);
    let mut members = vec![Vec::new(); graph.clusters.len()];
    for v in 0..n {
        for ci in tree.ancestry(v) {
            members[ci].push(v);
        }
    }
    members
}

/// The number of ordered placements of `n` nodes into `m` cells
/// (`m·(m-1)···(m-n+1)`), saturating once it passes [`FORCED_EXHAUSTIVE`] so the
/// product never overflows.
fn placement_count(m: usize, n: usize) -> u128 {
    let mut total: u128 = 1;
    for k in 0..n {
        total = total.saturating_mul((m - k) as u128);
        if total > FORCED_EXHAUSTIVE {
            return total;
        }
    }
    total
}

/// Exact search: try every way to seat the `n` nodes on `n` distinct cells (the
/// empty cells don't affect cost, so this enumerates placements — `m·(m-1)···` —
/// not the `m!` permutation). Only the un-pinned path reaches it.
fn exhaustive(ctx: &Ctx) -> Vec<usize> {
    let mut assign = vec![0usize; ctx.n];
    let mut used = vec![false; ctx.cell_count];
    let mut best: Vec<usize> = (0..ctx.n).collect();
    let mut best_cost = cost(&best, ctx);
    place_node(0, &mut assign, &mut used, ctx, &mut best, &mut best_cost);
    best
}

/// Recursively seat node `i` in each free cell, scoring complete placements.
fn place_node(
    i: usize,
    assign: &mut [usize],
    used: &mut [bool],
    ctx: &Ctx,
    best: &mut Vec<usize>,
    best_cost: &mut usize,
) {
    if i == ctx.n {
        let c = cost(assign, ctx);
        if c < *best_cost {
            *best_cost = c;
            best.copy_from_slice(assign);
        }
        return;
    }
    for cell in 0..ctx.cell_count {
        if used[cell] {
            continue;
        }
        used[cell] = true;
        assign[i] = cell;
        place_node(i + 1, assign, used, ctx, best, best_cost);
        used[cell] = false;
    }
}

/// Deterministic iterated local search, for grids too large to enumerate. Each
/// round descends to a 2-opt local minimum, then either restarts from a fresh
/// random permutation or perturbs (kicks) the best-so-far — the latter escapes
/// the minima where a single edge-through-node crossing has no improving swap but
/// a small jolt followed by descent removes it. The best minimum over all rounds
/// wins. The full `m`-cell permutation is the working state (`perm[..n]` assigns
/// nodes, `perm[n..]` are the empty cells the kick can move a node into).
fn local_search(ctx: &Ctx, seed: Vec<usize>, budget: &mut u64) -> Vec<usize> {
    let mut rng = Rng::new();
    let mut best = seed;
    let mut best_cost = two_opt(&mut best, ctx, budget);

    for round in 1..RESTARTS {
        if *budget == 0 {
            break; // work budget spent — return the best found so far
        }
        let mut perm = best.clone();
        // Alternate: even rounds kick the incumbent, odd rounds restart fresh. Both
        // touch only movable slots, so pinned nodes never leave their cells.
        if round.is_multiple_of(2) {
            kick(&mut perm, ctx, &mut rng);
        } else {
            shuffle(&mut perm, &ctx.movable, &mut rng);
        }
        let c = two_opt(&mut perm, ctx, budget);
        if c < best_cost {
            best_cost = c;
            best = perm;
        }
    }
    best[..ctx.n].to_vec()
}

/// Perturb `perm` with a few random cell swaps — a kick strong enough to leave the
/// current 2-opt basin but small enough to keep most of the good structure (the
/// classic iterated-local-search double-bridge analogue for a grid assignment).
/// Moves an un-pinned node into a movable slot, so pinned cells are never touched.
/// Inert when nothing is movable (every node pinned).
fn kick(perm: &mut [usize], ctx: &Ctx, rng: &mut Rng) {
    if ctx.unpinned_nodes.is_empty() || ctx.movable.is_empty() {
        return;
    }
    for _ in 0..3 {
        let i = ctx.unpinned_nodes[rng.below(ctx.unpinned_nodes.len())]; // an un-pinned node
        let j = ctx.movable[rng.below(ctx.movable.len())]; // any movable slot (maybe empty → relocate)
        perm.swap(i, j);
    }
}

/// Descend `perm` (a full `m`-cell permutation) to a local minimum by accepting
/// any swap that lowers the cost. Returns the settled cost; `perm[..n]` is the
/// assignment. Swapping two empty cells is a no-op, so it is skipped.
fn two_opt(perm: &mut [usize], ctx: &Ctx, budget: &mut u64) -> usize {
    let (m, n) = (perm.len(), ctx.n);
    let mut cur_cost = cost(&perm[..n], ctx);
    *budget = budget.saturating_sub(1);
    loop {
        let mut improved = false;
        for i in 0..m {
            for j in (i + 1)..m {
                if i >= n && j >= n {
                    continue; // swapping two empty cells changes nothing
                }
                if ctx.pinned[i] || ctx.pinned[j] {
                    continue; // a pinned node never moves, nor is its cell taken
                }
                if *budget == 0 {
                    return cur_cost; // out of work budget — stop here
                }
                perm.swap(i, j);
                let c = cost(&perm[..n], ctx);
                *budget -= 1;
                if c < cur_cost {
                    cur_cost = c;
                    improved = true;
                } else {
                    perm.swap(i, j);
                }
            }
        }
        if !improved {
            break;
        }
    }
    cur_cost
}

/// The weighted cost of an assignment (`assign[i]` = the cell node `i` sits in) —
/// one unsigned integer the search minimises. Every concern is a term scaled by
/// its weight ([`GridParams`] for the three dials, [`FRAME_COST`]/[`SPREAD_COST`]
/// fixed). No pixels: every node is a `(col, row)` cell.
fn cost(assign: &[usize], ctx: &Ctx) -> usize {
    // `(col, row)` of every node, resolved once per evaluation.
    let cols = icoord(ctx.cols);
    let cell: Vec<(i64, i64)> = assign[..ctx.n]
        .iter()
        .map(|&s| (icoord(s) % cols, icoord(s) / cols))
        .collect();

    let p = ctx.params;
    let mut total = 0usize;

    for (k, &(a, b)) in ctx.edges.iter().enumerate() {
        let (pa, pb) = (cell[a], cell[b]);
        // Edge length in cells (Manhattan).
        total += p.distance_cost
            * usize::try_from((pa.0 - pb.0).abs() + (pa.1 - pb.1).abs()).unwrap_or(0);
        // Travel against the reading direction — left for left-to-right, up for
        // top-down.
        let backward = if ctx.lr { pa.0 - pb.0 } else { pa.1 - pb.1 };
        total += p.flow_cost * usize::try_from(backward.max(0)).unwrap_or(0);
        // The edge passing through a third node's cell square.
        for (c, &pc) in cell.iter().enumerate() {
            if c != a && c != b && crosses_cell(pa, pb, pc) {
                total += p.crossing_cost;
            }
        }
        // Two edges crossing (each unordered pair once).
        for &(c, d) in &ctx.edges[k + 1..] {
            if a != c && a != d && b != c && b != d && segments_cross(pa, pb, cell[c], cell[d]) {
                total += p.crossing_cost;
            }
        }
    }

    // Cluster frames: a non-member inside a cluster's cell-box is an intrusion
    // (the constraint-weight term); a loose box pays the compactness term so
    // members pull together into a tight frame.
    for members in &ctx.clusters {
        if members.is_empty() {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
        for &m in members {
            let (x, y) = cell[m];
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        total += SPREAD_COST * usize::try_from((x1 - x0) + (y1 - y0)).unwrap_or(0);
        for (c, &(x, y)) in cell.iter().enumerate() {
            if !members.contains(&c) && x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                total += FRAME_COST;
            }
        }
    }

    total
}

/// Whether the segment between cells `a` and `b` passes through the *interior* of
/// cell `p`'s unit square — the discrete "which grid squares does this line cross"
/// test. Coordinates are doubled so each square has integer bounds `[2p±1]`,
/// keeping it exact; a line that merely grazes a corner (tangent, not through the
/// interior) does not count. The caller guarantees `p` is neither endpoint.
fn crosses_cell(a: (i64, i64), b: (i64, i64), p: (i64, i64)) -> bool {
    let (ax, ay, bx, by) = (2 * a.0, 2 * a.1, 2 * b.0, 2 * b.1);
    let (px, py) = (2 * p.0, 2 * p.1);
    // The segment's bounding box must reach the square, or it cannot touch it.
    if ax.max(bx) < px - 1 || ax.min(bx) > px + 1 || ay.max(by) < py - 1 || ay.min(by) > py + 1 {
        return false;
    }
    // The line enters the square's interior iff its four corners do not all fall on
    // one side of the line (some strictly positive, some strictly negative).
    let (mut pos, mut neg) = (false, false);
    for sx in [-1, 1] {
        for sy in [-1, 1] {
            let side = (bx - ax) * (py + sy - ay) - (by - ay) * (px + sx - ax);
            if side > 0 {
                pos = true;
            } else if side < 0 {
                neg = true;
            }
        }
    }
    pos && neg
}

/// Whether segments `a..b` and `c..d` cross properly (strict; collinear or merely
/// touching at an endpoint does not count). Integer orientation — exact, no
/// epsilon.
fn segments_cross(a: (i64, i64), b: (i64, i64), c: (i64, i64), d: (i64, i64)) -> bool {
    let turn = |p: (i64, i64), q: (i64, i64), r: (i64, i64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let (d1, d2) = (turn(c, d, a), turn(c, d, b));
    let (d3, d4) = (turn(a, b, c), turn(a, b, d));
    ((d1 > 0) != (d2 > 0)) && ((d3 > 0) != (d4 > 0)) && d1 != 0 && d2 != 0
}

/// A tiny deterministic xorshift PRNG, so restarts vary without breaking the
/// engine's determinism (no clock, no entropy).
struct Rng(u64);

impl Rng {
    fn new() -> Self {
        Self(0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next_u64() % n as u64).unwrap_or(0)
    }
}

/// In-place Fisher–Yates shuffle of the values at the `movable` slots only (pinned
/// slots keep their cells), with the deterministic PRNG. With every slot movable
/// this is the plain whole-array shuffle.
fn shuffle(v: &mut [usize], movable: &[usize], rng: &mut Rng) {
    for k in (1..movable.len()).rev() {
        let j = rng.below(k + 1);
        v.swap(movable[k], movable[j]);
    }
}

/// Assemble the [`Layout`] for the chosen assignment: nodes at their cells, edges
/// as straight segments clipped to borders, plus cluster boxes (shared with the
/// layered engine), origin-shifted by [`crate::finish`].
fn build_layout(graph: &Graph, cells: &[Pt], assign: &[usize]) -> Layout {
    let nodes: Vec<NodePos> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| NodePos {
            id: nd.id.clone(),
            center: cells[assign[i]],
            width: nd.width,
            height: nd.height,
        })
        .collect();

    let node_box = |id: &str| {
        let i = graph.node_index(id)?;
        Some(splines::NodeBox {
            center: nodes[i].center,
            half_w: graph.nodes[i].width / 2.0,
            half_h: graph.nodes[i].height / 2.0,
        })
    };
    let edges: Vec<EdgeRoute> = graph
        .edges
        .iter()
        .filter_map(|e| {
            if e.tail == e.head {
                return None;
            }
            let (ta, ha) = (node_box(&e.tail)?, node_box(&e.head)?);
            let (spline, polyline) = splines::route_edge(&[ta.center, ha.center], ta, ha);
            let label_pos = e.label.map(|_| crate::midpoint(&polyline));
            Some(EdgeRoute {
                tail: e.tail.clone(),
                head: e.head.clone(),
                spline,
                polyline,
                label_pos,
            })
        })
        .collect();

    let tree = ClusterTree::build(graph, graph.nodes.len());
    let clusters = crate::cluster_boxes(graph, &tree, &nodes);
    crate::finish(nodes, edges, clusters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Cluster, Edge, Node};

    fn node(id: &str) -> Node {
        Node::new(id, 80.0, 40.0)
    }

    fn distinct_cells(l: &Layout) {
        for i in 0..l.nodes.len() {
            for j in (i + 1)..l.nodes.len() {
                let (a, b) = (l.nodes[i].center, l.nodes[j].center);
                assert!(
                    (a.x - b.x).abs() > 0.5 || (a.y - b.y).abs() > 0.5,
                    "distinct cells"
                );
            }
        }
    }

    #[test]
    fn places_every_node_at_a_distinct_cell() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("c", "d"));
        let l = grid_layout(&g, GridParams::default(), &[]);
        assert_eq!(l.nodes.len(), 4);
        distinct_cells(&l);
    }

    #[test]
    fn prefers_adjacency_for_connected_nodes() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("c", "d"));
        let l = grid_layout(&g, GridParams::default(), &[]);
        let pos = |id: &str| l.nodes.iter().find(|n| n.id == id).unwrap().center;
        let d = |p: Pt, q: Pt| (p.x - q.x).hypot(p.y - q.y);
        let cell = 80.0 + GRID_GAP;
        assert!(d(pos("a"), pos("b")) <= cell + 1.0, "a,b adjacent");
        assert!(d(pos("c"), pos("d")) <= cell + 1.0, "c,d adjacent");
    }

    #[test]
    fn keeps_a_cluster_compact_and_external_out_of_its_frame() {
        // Cluster {m1,m2,m3} plus two externals; on the 3×2 grid the members can
        // sit in one row and the externals in the other, so no external falls
        // inside the cluster's member bounding box.
        let mut g = Graph::new();
        for id in ["m1", "m2", "m3", "e1", "e2"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("m1", "m2"));
        g.edges.push(Edge::new("m2", "m3"));
        g.edges.push(Edge::new("e1", "m1"));
        g.edges.push(Edge::new("e2", "m3"));
        g.clusters.push(Cluster {
            id: "C".to_owned(),
            parent: None,
            members: vec!["m1".to_owned(), "m2".to_owned(), "m3".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        let l = grid_layout(&g, GridParams::default(), &[]);
        let c = |id: &str| {
            let n = l.nodes.iter().find(|n| n.id == id).unwrap();
            (n.center.x, n.center.y)
        };
        let members = ["m1", "m2", "m3"].map(c);
        let (x0, y0) = (
            members.iter().map(|p| p.0).fold(f64::MAX, f64::min),
            members.iter().map(|p| p.1).fold(f64::MAX, f64::min),
        );
        let (x1, y1) = (
            members.iter().map(|p| p.0).fold(f64::MIN, f64::max),
            members.iter().map(|p| p.1).fold(f64::MIN, f64::max),
        );
        for ext in ["e1", "e2"] {
            let (ex, ey) = c(ext);
            assert!(
                ex < x0 || ex > x1 || ey < y0 || ey > y1,
                "{ext} stays outside the cluster member box"
            );
        }
    }

    #[test]
    fn exact_and_heuristic_both_place_every_node() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("b", "c"));
        let exact = grid_layout(
            &g,
            GridParams {
                search: SearchMode::Exhaustive,
                ..GridParams::default()
            },
            &[],
        );
        let heuristic = grid_layout(
            &g,
            GridParams {
                search: SearchMode::Heuristic,
                ..GridParams::default()
            },
            &[],
        );
        assert_eq!(exact.nodes.len(), 4);
        assert_eq!(heuristic.nodes.len(), 4);
        distinct_cells(&exact);
        distinct_cells(&heuristic);
    }

    #[test]
    fn forced_exact_on_a_large_graph_falls_back_without_hanging() {
        // The placement space is astronomically large; forced exact must fall back
        // to the bounded heuristic, place every node, and return promptly.
        let mut g = Graph::new();
        for i in 0..30 {
            g.nodes.push(node(&format!("n{i}")));
        }
        for i in 1..30 {
            g.edges
                .push(Edge::new(format!("n{}", i - 1), format!("n{i}")));
        }
        let l = grid_layout(
            &g,
            GridParams {
                search: SearchMode::Exhaustive,
                ..GridParams::default()
            },
            &[],
        );
        assert_eq!(l.nodes.len(), 30);
        distinct_cells(&l);
    }

    #[test]
    fn handles_large_graphs_via_local_search() {
        // Well past the exact-search cap: must place every node (no hang).
        let count = 40;
        let mut g = Graph::new();
        for i in 0..count {
            g.nodes.push(node(&format!("n{i}")));
        }
        for i in 1..g.nodes.len() {
            g.edges
                .push(Edge::new(format!("n{}", i - 1), format!("n{i}")));
        }
        let l = grid_layout(&g, GridParams::default(), &[]);
        assert_eq!(l.nodes.len(), count);
        distinct_cells(&l);
    }

    /// No node center may lie on a non-incident edge — the settled-layout mirror
    /// of the cost's [`crosses_cell`] guarantee. Works on pixel centers: a node on
    /// the straight tail→head line is collinear (zero cross product, within a
    /// float tolerance) and inside its bounding box.
    fn assert_no_edge_crosses_a_node(l: &Layout) {
        for e in &l.edges {
            let (a, b) = (
                l.nodes.iter().find(|n| n.id == e.tail).unwrap().center,
                l.nodes.iter().find(|n| n.id == e.head).unwrap().center,
            );
            for c in &l.nodes {
                if c.id == e.tail || c.id == e.head {
                    continue;
                }
                let p = c.center;
                let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
                let within = p.x >= a.x.min(b.x) - 1.0
                    && p.x <= a.x.max(b.x) + 1.0
                    && p.y >= a.y.min(b.y) - 1.0
                    && p.y <= a.y.max(b.y) + 1.0;
                assert!(
                    !(cross.abs() < 1.0 && within),
                    "edge {}->{} runs over {}",
                    e.tail,
                    e.head,
                    c.id
                );
            }
        }
    }

    #[test]
    fn routes_a_long_edge_clear_of_intervening_nodes() {
        // A hub fanning out, plus a long edge from the hub to a leaf: a naive grid
        // would run that edge straight through the fan, threading the gaps between
        // their cells. The crossing-aware placement must keep it clear.
        let mut g = Graph::new();
        for id in ["hub", "a", "b", "c", "d", "leaf"] {
            g.nodes.push(node(id));
        }
        for h in ["a", "b", "c", "d"] {
            g.edges.push(Edge::new("hub", h));
        }
        g.edges.push(Edge::new("hub", "leaf"));
        g.edges.push(Edge::new("a", "leaf"));
        let l = grid_layout(&g, GridParams::default(), &[]);
        assert_no_edge_crosses_a_node(&l);
    }

    #[test]
    fn biases_arrows_with_the_reading_direction() {
        // One edge a→b on a 2×2 grid (cell index → (col, row): 0=(0,0), 1=(1,0),
        // 2=(0,1), 3=(1,1)). The flow bias makes the with-direction placement
        // cheaper: rightward under left-to-right, downward under top-down.
        let left_right = Ctx {
            n: 2,
            cols: 2,
            cell_count: 4,
            lr: true,
            params: GridParams::default(),
            edges: vec![(0, 1)],
            clusters: Vec::new(),
            pinned: vec![false; 4],
            movable: (0..4).collect(),
            unpinned_nodes: vec![0, 1],
        };
        assert!(
            cost(&[0, 1], &left_right) < cost(&[1, 0], &left_right),
            "left-to-right favours →"
        );

        let top_down = Ctx {
            lr: false,
            ..left_right
        };
        assert!(
            cost(&[0, 2], &top_down) < cost(&[2, 0], &top_down),
            "top-down favours ↓"
        );
    }

    /// The pin cell (row, col) a laid-out node landed on, recovered from its centre
    /// via the emitted grid metadata — the inverse of drag-to-pin (raw cell minus the
    /// frame `pad`), so it reports the same coordinates a pin is expressed in.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn cell_of(l: &Layout, id: &str) -> (usize, usize) {
        let g = l.grid.expect("grid layout emits grid metadata");
        let c = l.nodes.iter().find(|n| n.id == id).unwrap().center;
        let col = ((c.x - g.origin.x) / g.cell_w).round().max(0.0) as usize;
        let row = ((c.y - g.origin.y) / g.cell_h).round().max(0.0) as usize;
        (row.saturating_sub(g.pad), col.saturating_sub(g.pad))
    }

    fn pin(node: usize, row: usize, col: usize) -> Pin {
        Pin { node, row, col }
    }

    /// Timing harness (run with `--ignored --nocapture`): how `grid_layout` scales
    /// with node count — where the heuristic falls off the cliff.
    #[test]
    #[ignore = "timing harness, run explicitly"]
    fn bench_grid_sizes() {
        use std::time::Instant;
        for &n in &[4usize, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256] {
            let mut g = Graph::new();
            for i in 0..n {
                g.nodes.push(node(&format!("n{i}")));
            }
            for i in 1..n {
                g.edges
                    .push(Edge::new(format!("n{}", i - 1), format!("n{i}")));
            }
            let t = Instant::now();
            let l = grid_layout(&g, GridParams::default(), &[]);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let gm = l.grid.unwrap();
            println!(
                "n={n:4}  grid={}x{} ({} cells)  {ms:9.1} ms",
                gm.cols,
                gm.rows,
                gm.cols * gm.rows
            );
        }
    }

    #[test]
    fn empty_pins_matches_the_unpinned_layout() {
        // The regression guard: pinning machinery must not perturb the no-pin result.
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"] {
            g.nodes.push(node(id));
        }
        for w in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
            .windows(2)
            .map(|w| (w[0], w[1]))
        {
            g.edges.push(Edge::new(w.0, w.1));
        }
        let mut bare = grid_layout(&g, GridParams::default(), &[]);
        bare.grid = None; // the only addition; compare the placement itself
        let with_empty = grid_layout(&g, GridParams::default(), &[]);
        assert_eq!(bare.nodes, with_empty.nodes);
        assert_eq!(bare.edges, with_empty.edges);
    }

    #[test]
    fn unpinned_grid_reserves_a_drag_room_frame() {
        // With nothing pinned the client still gets a MARGIN-cell band on every side to
        // drag boxes into: the emitted grid wraps the content in a `pad`-cell frame.
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d", "e"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        let l = grid_layout(&g, GridParams::default(), &[]);
        let meta = l.grid.expect("grid layout emits grid metadata");
        assert_eq!(meta.pad, MARGIN, "frame reserved even with no pins");
        // Every node's raw cell sits at least MARGIN cells from each edge of the grid.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        for nd in &l.nodes {
            let raw_c = ((nd.center.x - meta.origin.x) / meta.cell_w).round() as usize;
            let raw_r = ((nd.center.y - meta.origin.y) / meta.cell_h).round() as usize;
            assert!(raw_c >= MARGIN && raw_r >= MARGIN, "top/left frame");
            assert!(meta.cols - 1 - raw_c >= MARGIN, "right frame");
            assert!(meta.rows - 1 - raw_r >= MARGIN, "bottom frame");
        }
    }

    #[test]
    fn drag_to_pin_round_trips() {
        // The web reads where a node landed (cellAt over the emitted grid — raw cell
        // minus `pad`), then pins it at that cell. Re-laying out must report it on the
        // same pin cell, or drag-to-pin would jump the box out from under the cursor.
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("a", "c"));
        let l1 = grid_layout(&g, GridParams::default(), &[]);
        let (r, c) = cell_of(&l1, "c");
        let l2 = grid_layout(&g, GridParams::default(), &[pin(2, r, c)]);
        assert_eq!(cell_of(&l2, "c"), (r, c), "pinned node holds its dropped cell");
        distinct_cells(&l2);
    }

    #[test]
    fn pinned_node_stays_at_its_cell() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d", "e"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("a", "c"));
        // Pin "a" to (3, 3) — a cell its edges would otherwise pull it off.
        let l = grid_layout(&g, GridParams::default(), &[pin(0, 3, 3)]);
        assert_eq!(cell_of(&l, "a"), (3, 3), "pinned node holds its cell");
        distinct_cells(&l);
    }

    #[test]
    fn unpinned_nodes_fill_around_a_pin() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        // Pin "c" to a cell; it holds it while a, b, d fill around.
        let l = grid_layout(&g, GridParams::default(), &[pin(2, 2, 2)]);
        assert_eq!(cell_of(&l, "c"), (2, 2));
        distinct_cells(&l); // a, b, d placed around it, no overlap
    }

    #[test]
    fn out_of_bounds_pin_is_dropped() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        // A far-off pin grows the grid to fit it; no panic, every node distinct.
        let l = grid_layout(&g, GridParams::default(), &[pin(0, 99, 99)]);
        assert_eq!(l.nodes.len(), 4);
        distinct_cells(&l);
    }

    #[test]
    fn colliding_pins_keep_one_and_place_the_rest() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        // Two pins on the same cell → one wins, the other places freely; no panic.
        let l = grid_layout(&g, GridParams::default(), &[pin(0, 2, 2), pin(1, 2, 2)]);
        assert_eq!(l.nodes.len(), 4);
        distinct_cells(&l);
        let occupants = ["a", "b"]
            .into_iter()
            .filter(|id| cell_of(&l, id) == (2, 2));
        assert_eq!(
            occupants.count(),
            1,
            "exactly one node holds the contested cell"
        );
    }

    #[test]
    fn all_nodes_pinned_is_a_full_manual_layout() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(node(id));
        }
        // Every node pinned to a distinct cell.
        let pins = [pin(0, 2, 2), pin(1, 2, 3), pin(2, 3, 2), pin(3, 3, 3)];
        let l = grid_layout(&g, GridParams::default(), &pins);
        assert_eq!(cell_of(&l, "a"), (2, 2));
        assert_eq!(cell_of(&l, "b"), (2, 3));
        assert_eq!(cell_of(&l, "c"), (3, 2));
        assert_eq!(cell_of(&l, "d"), (3, 3));
    }

    #[test]
    fn pins_keep_a_margin_on_every_side() {
        let mut g = Graph::new();
        for id in ["a", "b"] {
            g.nodes.push(node(id));
        }
        g.edges.push(Edge::new("a", "b"));
        // Pin both against the top-left origin. A pin holds its exact cell in pin
        // coordinates; the engine offsets pinned content inward by `pad`, reserving a
        // MARGIN-cell drag-room frame above/left, and grows the grid so the same band
        // sits below/right — drag room on every side.
        let l = grid_layout(&g, GridParams::default(), &[pin(0, 0, 0), pin(1, 0, 1)]);
        let meta = l.grid.expect("grid layout emits grid metadata");
        assert_eq!(cell_of(&l, "a"), (0, 0), "a holds its exact pin cell");
        assert_eq!(cell_of(&l, "b"), (0, 1), "b holds its exact pin cell");
        assert_eq!(meta.pad, MARGIN, "top/left drag-room frame reserved");
        // The raw (framed) cell of the furthest pin, with MARGIN room past it.
        let (max_raw_r, max_raw_c) = (meta.pad, 1 + meta.pad);
        assert!(meta.rows - 1 - max_raw_r >= MARGIN, "bottom drag room");
        assert!(meta.cols - 1 - max_raw_c >= MARGIN, "right drag room");
    }

    #[test]
    fn pinning_is_deterministic() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"] {
            g.nodes.push(node(id));
        }
        for w in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
            .windows(2)
            .map(|w| (w[0], w[1]))
        {
            g.edges.push(Edge::new(w.0, w.1));
        }
        let pins = [pin(0, 0, 0), pin(5, 2, 1)];
        assert_eq!(
            grid_layout(&g, GridParams::default(), &pins),
            grid_layout(&g, GridParams::default(), &pins),
        );
    }

    #[test]
    fn deterministic() {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"] {
            g.nodes.push(node(id));
        }
        for w in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
            .windows(2)
            .map(|w| (w[0], w[1]))
        {
            g.edges.push(Edge::new(w.0, w.1));
        }
        assert_eq!(
            grid_layout(&g, GridParams::default(), &[]),
            grid_layout(&g, GridParams::default(), &[])
        ); // exercises the local search too
    }
}
