//! X-coordinate assignment — `dot`'s positioning pass (`lib/dotgen/position.c`,
//! Gansner et al. 1993 §4.2).
//!
//! Coordinates along the within-rank axis are found by network simplex on an
//! *auxiliary* graph: for each layout edge `(u, v)` a slack node `e` with edges
//! `e → u` and `e → v` of weight `Ω·w` turns the objective into `Σ Ω·w·|x_u −
//! x_v|` — i.e. minimise total edge "horizontal" length, keeping nodes aligned
//! with their neighbours and long edges straight. The `Ω` factor (1 real–real,
//! 2 real–virtual, 8 virtual–virtual) biases straightness onto long edges.
//! Left-to-right separation edges within each rank enforce ordering and minimum
//! gaps.

use crate::mincross::Ordered;
use crate::ns::{Balance, Constraint, rank};

/// Assign the within-rank coordinate (centre) of every vnode, in points.
///
/// `minor_width[v]` is the node's size along the within-rank axis; `nodesep` the
/// minimum gap between neighbours. Returns one coordinate per vnode, normalised
/// so the leftmost node sits at `nodesep`.
pub(crate) fn assign_minor(ordered: &Ordered, minor_width: &[f64], nodesep: f64) -> Vec<f64> {
    let v = ordered.vnodes.len();
    let mut cs: Vec<Constraint> = Vec::new();

    // Separation constraints: each left neighbour precedes its right neighbour by
    // at least half-widths plus nodesep.
    for row in &ordered.ranks {
        for pair in row.windows(2) {
            let (l, r) = (pair[0], pair[1]);
            let gap = (minor_width[l] / 2.0 + nodesep + minor_width[r] / 2.0).round();
            cs.push(Constraint {
                tail: l,
                head: r,
                minlen: i32_of(gap).max(1),
                weight: 0,
            });
        }
    }

    // One slack node per layout-edge segment, with weighted edges to both
    // endpoints; the simplex pulls the slack node up to `min(x_u, x_v)`, so its
    // two edges cost `Ω·|x_u − x_v|`.
    let mut aux = v;
    for chain in &ordered.chains {
        for seg in chain.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let omega = omega(
                ordered.vnodes[a].real.is_none(),
                ordered.vnodes[b].real.is_none(),
            );
            cs.push(Constraint {
                tail: aux,
                head: a,
                minlen: 0,
                weight: omega,
            });
            cs.push(Constraint {
                tail: aux,
                head: b,
                minlen: 0,
                weight: omega,
            });
            aux += 1;
        }
    }

    let solved = rank(aux, &cs, Balance::None);
    let min_x = (0..v).map(|i| solved[i]).min().unwrap_or(0);
    (0..v)
        .map(|i| f64::from(solved[i] - min_x) + nodesep)
        .collect()
}

/// The `Ω` straightness weight for an edge by endpoint kind (TSE 1993 §4.2):
/// real–real 1, real–virtual 2, virtual–virtual 8.
fn omega(a_virtual: bool, b_virtual: bool) -> i32 {
    match (a_virtual, b_virtual) {
        (true, true) => 8,
        (false, false) => 1,
        _ => 2,
    }
}

/// Round a non-negative `f64` to `i32`, saturating (separation gaps are small
/// and positive).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn i32_of(x: f64) -> i32 {
    x.max(0.0).min(f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge as GEdge, Graph, Node};
    use crate::mincross::order;
    use crate::rank::assign_ranks;

    fn graph(ids: &[&str], edges: &[(&str, &str)]) -> Graph {
        let mut g = Graph::new();
        g.nodes = ids.iter().map(|i| Node::new(*i, 72.0, 36.0)).collect();
        g.edges = edges.iter().map(|(t, h)| GEdge::new(*t, *h)).collect();
        g
    }

    fn widths(o: &Ordered, g: &Graph) -> Vec<f64> {
        o.vnodes
            .iter()
            .map(|vn| vn.real.map_or(1.0, |i| g.nodes[i].width))
            .collect()
    }

    #[test]
    fn order_is_preserved_left_to_right() {
        let g = graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let r = assign_ranks(&g);
        let o = order(&g, &r, 1.0);
        let x = assign_minor(&o, &widths(&o, &g), 18.0);
        for row in &o.ranks {
            for pair in row.windows(2) {
                assert!(x[pair[0]] < x[pair[1]], "x increases along the rank order");
            }
        }
    }

    #[test]
    fn min_gap_respected() {
        // Two siblings on one rank must be at least nodesep + widths apart.
        let g = graph(&["root", "a", "b"], &[("root", "a"), ("root", "b")]);
        let ranking = assign_ranks(&g);
        let ord = order(&g, &ranking, 1.0);
        let xs = assign_minor(&ord, &widths(&ord, &g), 18.0);
        let ai = g.node_index("a").unwrap();
        let bi = g.node_index("b").unwrap();
        let gap = (xs[ai] - xs[bi]).abs();
        assert!(gap >= 72.0 + 18.0 - 0.5, "siblings separated: {gap}");
    }

    #[test]
    fn long_edge_is_straightened() {
        // a -> b -> c and a -> c: the virtual node on the skip edge should sit
        // close to the straight line a..c rather than be pulled aside.
        let g = graph(&["a", "b", "c"], &[("a", "b"), ("b", "c"), ("a", "c")]);
        let r = assign_ranks(&g);
        let o = order(&g, &r, 1.0);
        let x = assign_minor(&o, &widths(&o, &g), 18.0);
        let virt = o.chains[2][1]; // a -> virtual -> c
        let ai = g.node_index("a").unwrap();
        let ci = g.node_index("c").unwrap();
        // a and c are vertically aligned (single chain), so the virtual is too.
        assert!((x[virt] - x[ai]).abs() < 1.0 && (x[virt] - x[ci]).abs() < 1.0);
    }

    #[test]
    fn deterministic() {
        let g = graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let r = assign_ranks(&g);
        let o = order(&g, &r, 1.0);
        let w = widths(&o, &g);
        assert_eq!(assign_minor(&o, &w, 18.0), assign_minor(&o, &w, 18.0));
    }
}
