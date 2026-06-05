//! Test-only fidelity oracle: drive the real Graphviz `dot` binary and parse
//! its geometry, so the Rust port can be cross-validated against ground truth.
//!
//! `dot -Tjson` reports node centres (`pos`) and edge b-splines (`_draw_`) in
//! **points with y growing up** (origin bottom-left). This module converts both
//! to the engine's convention (points, y-down, origin top-left) by flipping
//! about the graph bounding-box height, so a comparison is apples-to-apples.
//!
//! Every entry point returns `None` when `dot` is absent, letting callers skip
//! (rather than fail) on a machine without Graphviz.

#![cfg(test)]
// The harness is built out in Phase 0; rank/order recovery and the geometry
// fields are consumed by the per-pass oracle tests added in Phases 2–5.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::graph::{Graph, RankDir};

/// Points per inch — `dot` reports node sizes in inches, positions in points.
const PT_PER_IN: f64 = 72.0;

/// A node as `dot` placed it (points, y-down): centre + size.
#[derive(Debug, Clone)]
pub(crate) struct OracleNode {
    pub name: String,
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
}

/// An edge as `dot` routed it (points, y-down): the b-spline control points.
#[derive(Debug, Clone)]
pub(crate) struct OracleEdge {
    pub tail: String,
    pub head: String,
    pub points: Vec<(f64, f64)>,
}

/// A cluster bounding box as `dot` computed it (points, y-down, top-left origin).
#[derive(Debug, Clone)]
pub(crate) struct OracleCluster {
    /// The cluster id (the `cluster_` prefix stripped).
    pub id: String,
    pub bbox: crate::layout::Box2,
}

/// A parsed `dot` layout, in the engine's coordinate convention.
#[derive(Debug, Clone)]
pub(crate) struct Oracle {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<OracleNode>,
    pub edges: Vec<OracleEdge>,
    pub clusters: Vec<OracleCluster>,
}

impl Oracle {
    /// The bounding box `dot` gave the cluster with this id, if present.
    pub(crate) fn cluster_bb(&self, id: &str) -> Option<crate::layout::Box2> {
        self.clusters.iter().find(|c| c.id == id).map(|c| c.bbox)
    }
}

impl Oracle {
    /// Recover each node's rank from its row: in a TB layout, nodes sharing a
    /// y-centre (within tolerance) are on one rank; ranks number from the top
    /// (smallest y) downward.
    pub(crate) fn ranks(&self) -> HashMap<String, usize> {
        let mut rows: Vec<f64> = self.nodes.iter().map(|n| n.cy).collect();
        rows.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut distinct: Vec<f64> = Vec::new();
        for y in rows {
            if !distinct.iter().any(|d| (d - y).abs() < 1.0) {
                distinct.push(y);
            }
        }
        self.nodes
            .iter()
            .map(|n| {
                let rank = distinct
                    .iter()
                    .position(|d| (d - n.cy).abs() < 1.0)
                    .unwrap_or(0);
                (n.name.clone(), rank)
            })
            .collect()
    }

    /// The left-to-right order of node names within each rank (by x-centre).
    pub(crate) fn order(&self) -> Vec<Vec<String>> {
        let ranks = self.ranks();
        let max_rank = ranks.values().copied().max().unwrap_or(0);
        (0..=max_rank)
            .map(|r| {
                let mut row: Vec<&OracleNode> = self
                    .nodes
                    .iter()
                    .filter(|n| ranks.get(&n.name) == Some(&r))
                    .collect();
                row.sort_by(|a, b| a.cx.partial_cmp(&b.cx).unwrap_or(std::cmp::Ordering::Equal));
                row.into_iter().map(|n| n.name.clone()).collect()
            })
            .collect()
    }
}

/// Whether the `dot` binary is available on this machine.
pub(crate) fn available() -> bool {
    Command::new("dot")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Render `graph` as a DOT source string equivalent to the engine's input —
/// fixed node sizes (inches), the same spacing, rankdir, and clusters.
pub(crate) fn to_dot(graph: &Graph) -> String {
    let rankdir = match graph.rankdir {
        RankDir::TopBottom => "TB",
        RankDir::LeftRight => "LR",
    };
    let mut s = String::from("digraph {\n");
    let _ = writeln!(s, "  rankdir={rankdir};");
    let _ = writeln!(
        s,
        "  nodesep={:.4}; ranksep={:.4};",
        graph.nodesep / PT_PER_IN,
        graph.ranksep / PT_PER_IN
    );
    s.push_str("  node [shape=box, fixedsize=true];\n");
    for n in &graph.nodes {
        let _ = writeln!(
            s,
            "  {} [width={:.4}, height={:.4}];",
            quote(&n.id),
            n.width / PT_PER_IN,
            n.height / PT_PER_IN
        );
    }
    let tree = crate::cluster::ClusterTree::build(graph, graph.nodes.len());
    for &root in &tree.roots {
        emit_cluster(&mut s, graph, &tree, root, 1);
    }
    for e in &graph.edges {
        let _ = writeln!(
            s,
            "  {} -> {} [minlen={}, weight={}];",
            quote(&e.tail),
            quote(&e.head),
            e.minlen,
            e.weight
        );
    }
    s.push_str("}\n");
    s
}

/// Emit one cluster as a nested `subgraph cluster_<id>`, recursing into child
/// clusters first so `dot` sees true nesting, then declaring direct members.
fn emit_cluster(
    s: &mut String,
    graph: &Graph,
    tree: &crate::cluster::ClusterTree,
    ci: usize,
    depth: usize,
) {
    let pad = "  ".repeat(depth);
    let _ = writeln!(
        s,
        "{pad}subgraph {} {{",
        quote(&format!("cluster_{}", graph.clusters[ci].id))
    );
    for &child in &tree.children[ci] {
        emit_cluster(s, graph, tree, child, depth + 1);
    }
    for m in &graph.clusters[ci].members {
        let _ = writeln!(s, "{pad}  {};", quote(m));
    }
    let _ = writeln!(s, "{pad}}}");
}

/// Run `dot -Tjson` on `graph` and parse the result, or `None` if `dot` is
/// unavailable or fails.
pub(crate) fn run(graph: &Graph) -> Option<Oracle> {
    if !available() {
        return None;
    }
    let src = to_dot(graph);
    let mut child = Command::new("dot")
        .arg("-Tjson")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(src.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&serde_json::from_slice(&out.stdout).ok()?)
}

/// Parse `dot -Tjson`, flipping y about the graph height into y-down points.
fn parse(json: &Value) -> Option<Oracle> {
    let bb = parse_floats(json.get("bb")?.as_str()?);
    let (height, width) = (bb.get(3).copied()?, bb.get(2).copied()?);
    let flip = |y: f64| height - y;

    // gvid -> node name, for resolving edge endpoints.
    let mut by_gvid: HashMap<u64, String> = HashMap::new();
    let mut nodes = Vec::new();
    let mut clusters = Vec::new();
    for obj in json.get("objects")?.as_array()? {
        let name = obj.get("name")?.as_str()?.to_owned();
        if let Some(gvid) = obj.get("_gvid").and_then(Value::as_u64) {
            by_gvid.insert(gvid, name.clone());
        }
        // Clusters have a `bb` (llx,lly,urx,ury) but no `pos`; record their box.
        let Some(pos) = obj.get("pos").and_then(Value::as_str) else {
            if let (Some(id), Some(cbb)) = (
                name.strip_prefix("cluster_"),
                obj.get("bb").and_then(Value::as_str).map(parse_floats),
            ) && cbb.len() == 4
            {
                let (llx, lly, urx, ury) = (cbb[0], cbb[1], cbb[2], cbb[3]);
                clusters.push(OracleCluster {
                    id: id.to_owned(),
                    // Flip y: dot's upper-right y becomes the top in y-down.
                    bbox: crate::layout::Box2::new(llx, flip(ury), urx - llx, ury - lly),
                });
            }
            continue;
        };
        let p = parse_floats(pos);
        let (cx, cy) = (p.first().copied()?, flip(p.get(1).copied()?));
        let w = obj
            .get("width")
            .and_then(Value::as_str)?
            .parse::<f64>()
            .ok()?
            * PT_PER_IN;
        let h = obj
            .get("height")
            .and_then(Value::as_str)?
            .parse::<f64>()
            .ok()?
            * PT_PER_IN;
        nodes.push(OracleNode { name, cx, cy, w, h });
    }

    let mut edges = Vec::new();
    for e in json.get("edges")?.as_array()? {
        let tail = by_gvid.get(&e.get("tail")?.as_u64()?)?.clone();
        let head = by_gvid.get(&e.get("head")?.as_u64()?)?.clone();
        let mut points = Vec::new();
        for draw in e
            .get("_draw_")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if draw.get("op").and_then(Value::as_str) == Some("b") {
                for pt in draw
                    .get("points")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let xy = pt.as_array()?;
                    points.push((xy.first()?.as_f64()?, flip(xy.get(1)?.as_f64()?)));
                }
            }
        }
        edges.push(OracleEdge { tail, head, points });
    }

    Some(Oracle {
        width,
        height,
        nodes,
        edges,
        clusters,
    })
}

/// Parse a comma-separated float list (`dot`'s `pos`/`bb` encoding).
fn parse_floats(s: &str) -> Vec<f64> {
    s.split(',')
        .filter_map(|t| t.trim().parse::<f64>().ok())
        .collect()
}

/// Quote a DOT identifier, escaping embedded quotes.
fn quote(id: &str) -> String {
    format!("\"{}\"", id.replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Node};

    fn diamond() -> Graph {
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(Node::new(id, 72.0, 36.0));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("a", "c"));
        g.edges.push(Edge::new("b", "d"));
        g.edges.push(Edge::new("c", "d"));
        g
    }

    #[test]
    fn to_dot_emits_sizes_and_edges() {
        let dot = to_dot(&diamond());
        assert!(dot.contains("rankdir=TB"));
        assert!(dot.contains("\"a\" -> \"b\""));
        assert!(dot.contains("width=1.0000")); // 72pt / 72 = 1in
    }

    #[test]
    fn to_dot_nests_subgraphs() {
        use crate::graph::Cluster;
        let mut g = diamond();
        g.clusters.push(Cluster {
            id: "outer".to_owned(),
            parent: None,
            members: vec!["d".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        g.clusters.push(Cluster {
            id: "inner".to_owned(),
            parent: Some("outer".to_owned()),
            members: vec!["b".to_owned(), "c".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        let dot = to_dot(&g);
        // The inner subgraph opens before it closes, inside the outer's braces.
        let outer_at = dot.find("cluster_outer").expect("outer emitted");
        let inner_at = dot.find("cluster_inner").expect("inner emitted");
        assert!(inner_at > outer_at, "inner nested after outer opens");
    }

    #[test]
    fn oracle_parses_nested_cluster_boxes() {
        use crate::graph::Cluster;
        // outer { inner { a -> b } ; c }, with b -> c so c sits below.
        let mut g = Graph::new();
        for id in ["a", "b", "c"] {
            g.nodes.push(Node::new(id, 72.0, 36.0));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("b", "c"));
        g.clusters.push(Cluster {
            id: "outer".to_owned(),
            parent: None,
            members: vec!["c".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        g.clusters.push(Cluster {
            id: "inner".to_owned(),
            parent: Some("outer".to_owned()),
            members: vec!["a".to_owned(), "b".to_owned()],
            margin: 8.0,
            header: 0.0,
        });
        let Some(o) = run(&g) else {
            eprintln!("dot not installed — skipping oracle test");
            return;
        };
        let outer = o.cluster_bb("outer").expect("outer bb");
        let inner = o.cluster_bb("inner").expect("inner bb");
        // The outer box strictly encloses the inner box (y-down).
        assert!(
            outer.x <= inner.x + 0.5,
            "outer left of inner: {outer:?} {inner:?}"
        );
        assert!(outer.y <= inner.y + 0.5, "outer above inner");
        assert!(
            outer.x + outer.w >= inner.x + inner.w - 0.5,
            "outer right of inner"
        );
        assert!(
            outer.y + outer.h >= inner.y + inner.h - 0.5,
            "outer below inner"
        );
    }

    #[test]
    fn oracle_recovers_diamond_ranks() {
        let Some(o) = run(&diamond()) else {
            eprintln!("dot not installed — skipping oracle test");
            return;
        };
        let ranks = o.ranks();
        // a at top, d at bottom, b and c level between them.
        assert_eq!(ranks["a"], 0);
        assert_eq!(ranks["b"], 1);
        assert_eq!(ranks["c"], 1);
        assert_eq!(ranks["d"], 2);
        // y-down: a's centre is above d's.
        let y = |n: &str| o.nodes.iter().find(|x| x.name == n).unwrap().cy;
        assert!(y("a") < y("d"));
    }
}
