//! Cycle breaking — `dot`'s first pass (`lib/dotgen/acyclic.c`). The ranking
//! solver needs a DAG, so back-edges are detected by a depth-first search and
//! marked *reversed*: the layout treats a reversed edge as `head → tail`, and
//! the caller flips the routed polyline back at draw time so the arrowhead still
//! lands on the true head.
//!
//! Deterministic: nodes are entered in index order and out-edges in edge-list
//! order, so the same input always reverses the same edges.

/// A DFS colour: a node on the current stack is `Gray`, a fully-explored one
/// `Black`; an edge into a `Gray` node closes a cycle.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Gray,
    Black,
}

/// Detect back-edges in the directed graph `(n nodes, edges as (tail, head))`,
/// returning a per-edge flag: `true` where the edge closes a cycle and must be
/// reversed for layering. Self-loops are never reversed (they carry no rank
/// constraint and are handled separately by routing).
#[must_use]
pub(crate) fn break_cycles(n: usize, edges: &[(usize, usize)]) -> Vec<bool> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, &(t, h)) in edges.iter().enumerate() {
        if t != h {
            adj[t].push(i);
        }
    }

    let mut color: Vec<Option<Mark>> = vec![None; n];
    let mut reversed = vec![false; edges.len()];

    // Iterative DFS. Frame: (node, cursor into adj[node]).
    for start in 0..n {
        if color[start].is_some() {
            continue;
        }
        color[start] = Some(Mark::Gray);
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(&mut (v, ref mut cursor)) = stack.last_mut() {
            if *cursor < adj[v].len() {
                let e = adj[v][*cursor];
                *cursor += 1;
                let w = edges[e].1;
                match color[w] {
                    Some(Mark::Gray) => reversed[e] = true, // back-edge
                    Some(Mark::Black) => {}                 // forward / cross edge
                    None => {
                        color[w] = Some(Mark::Gray);
                        stack.push((w, 0));
                    }
                }
            } else {
                color[v] = Some(Mark::Black);
                stack.pop();
            }
        }
    }
    reversed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dag_reverses_nothing() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        assert_eq!(break_cycles(4, &edges), vec![false; 4]);
    }

    #[test]
    fn two_cycle_reverses_one() {
        let edges = [(0, 1), (1, 0)];
        let r = break_cycles(2, &edges);
        assert_eq!(r.iter().filter(|&&x| x).count(), 1, "exactly one reversed");
    }

    #[test]
    fn three_cycle_reverses_one() {
        let edges = [(0, 1), (1, 2), (2, 0)];
        let r = break_cycles(3, &edges);
        assert_eq!(r, vec![false, false, true]);
    }

    #[test]
    fn self_loop_never_reversed() {
        let edges = [(0, 0), (0, 1)];
        assert_eq!(break_cycles(2, &edges), vec![false, false]);
    }

    #[test]
    fn deterministic() {
        let edges = [(0, 1), (1, 2), (2, 0), (0, 2), (2, 1)];
        assert_eq!(break_cycles(3, &edges), break_cycles(3, &edges));
    }
}
