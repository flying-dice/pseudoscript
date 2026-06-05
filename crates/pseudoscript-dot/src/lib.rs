//! `pseudoscript-dot` — a pure-Rust, wasm-safe port of the Graphviz `dot`
//! layered layout engine.
//!
//! A faithful port of `dot`'s four-pass pipeline (Gansner, Koutsofios, North,
//! Vo, *"A Technique for Drawing Directed Graphs"*, IEEE TSE 1993):
//!
//! 1. **rank** — assign each node an integer rank (network simplex).
//! 2. **order** — order nodes within ranks to minimise edge crossings.
//! 3. **position** — assign x-coordinates (network simplex on an auxiliary
//!    graph) and y-coordinates per rank.
//! 4. **splines** — route each edge as a piecewise Bézier through the layout.
//!
//! Clusters (`dot`'s `subgraph cluster_*`) are laid out on contiguous ranks
//! inside a bounding box. The engine is generic: a client builds a [`Graph`]
//! and reads back a [`Layout`]; C4 diagrams are the first client.
//!
//! Fidelity is checked against the real `dot` binary as a test oracle (see the
//! `oracle` module); those tests are skipped when `dot` is not installed.

mod acyclic;
pub mod graph;
pub mod layout;
mod mincross;
mod ns;
pub mod optimize;
mod position;
mod rank;
mod splines;

#[cfg(test)]
mod oracle;

pub use graph::{Cluster, Edge, Graph, Node, RankDir};
pub use layout::{Box2, ClusterBox, EdgeRoute, Layout, NodePos, Pt};

/// Width reserved for a virtual (long-edge routing) node along the within-rank
/// axis — a thin lane between real nodes.
const VIRTUAL_WIDTH: f64 = 1.0;

/// Lay `graph` out, returning placed nodes, routed edges, cluster boxes, and the
/// overall bounding box.
///
/// Deterministic: identical input yields identical output. Never panics — an
/// empty graph returns an empty [`Layout`].
///
/// Runs the full pipeline: rank (network simplex + cluster banding) → order
/// (crossing minimisation) → position (network-simplex x-coordinates) → splines
/// (Bézier edge routing), then cluster bounding boxes.
#[must_use]
pub fn layout(graph: &Graph) -> Layout {
    if graph.nodes.is_empty() {
        return Layout::default();
    }

    let ranking = rank::assign_ranks(graph);
    let ordered = mincross::order(graph, &ranking, VIRTUAL_WIDTH);

    let lr = graph.rankdir == RankDir::LeftRight;
    // "major" axis = rank direction; "minor" = within-rank. Real nodes project
    // their size onto each axis by rankdir; virtual nodes are thin lanes.
    let major_size = |v: usize| match ordered.vnodes[v].real {
        Some(i) if lr => graph.nodes[i].width,
        Some(i) => graph.nodes[i].height,
        None => 0.0,
    };
    let minor_width: Vec<f64> = (0..ordered.vnodes.len())
        .map(|v| {
            ordered.vnodes[v].real.map_or(VIRTUAL_WIDTH, |i| {
                if lr {
                    graph.nodes[i].height
                } else {
                    graph.nodes[i].width
                }
            })
        })
        .collect();

    // Major-axis centre per rank: stack rank thicknesses with ranksep.
    let mut row_major: Vec<f64> = vec![0.0; ordered.ranks.len()];
    let mut major = graph.ranksep;
    for (r, row) in ordered.ranks.iter().enumerate() {
        let thick = row.iter().map(|&v| major_size(v)).fold(0.0_f64, f64::max);
        row_major[r] = major + thick / 2.0;
        major += thick + graph.ranksep;
    }

    // Minor axis: network-simplex x-coordinates (alignment + straight long edges).
    let minor = position::assign_minor(&ordered, &minor_width, graph.nodesep);
    let mut center = vec![Pt::new(0.0, 0.0); ordered.vnodes.len()];
    for (r, row) in ordered.ranks.iter().enumerate() {
        for &v in row {
            center[v] = if lr {
                Pt::new(row_major[r], minor[v])
            } else {
                Pt::new(minor[v], row_major[r])
            };
        }
    }
    contain_externals(graph, &ordered, lr, &mut center);

    let nodes: Vec<NodePos> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| NodePos {
            id: nd.id.clone(),
            center: center[i],
            width: nd.width,
            height: nd.height,
        })
        .collect();

    // Each edge is a Bézier fit through its virtual-node corridor, clipped to the
    // endpoint borders. A reversed edge's chain runs in increasing-rank order
    // (head→tail), so flip it back to tail→head first.
    let node_box = |id: &str| {
        let i = graph.node_index(id)?;
        Some(splines::NodeBox {
            center: center[i],
            half_w: graph.nodes[i].width / 2.0,
            half_h: graph.nodes[i].height / 2.0,
        })
    };
    let edges: Vec<EdgeRoute> = graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let chain = &ordered.chains[i];
            if chain.is_empty() {
                return None;
            }
            let mut pts: Vec<Pt> = chain.iter().map(|&v| center[v]).collect();
            if ordered.reversed[i] {
                pts.reverse();
            }
            let (spline, polyline) =
                splines::route_edge(&pts, node_box(&e.tail)?, node_box(&e.head)?);
            let label_pos = e.label.map(|_| midpoint(&polyline));
            Some(EdgeRoute {
                tail: e.tail.clone(),
                head: e.head.clone(),
                spline,
                polyline,
                label_pos,
            })
        })
        .collect();

    let clusters = cluster_boxes(graph, &nodes);
    finish(nodes, edges, clusters)
}

/// Keep a non-member node that a same-rank move placed within a cluster's rank
/// band clear of that cluster's cross-axis span, so an external (e.g. a system
/// pulled up beside a container) still renders outside the boundary frame. Shifts
/// the node just past the nearer edge of the span along the within-rank axis.
fn contain_externals(graph: &Graph, ordered: &mincross::Ordered, lr: bool, center: &mut [Pt]) {
    let n = graph.nodes.len();
    let minor = |c: Pt| if lr { c.y } else { c.x };
    let size = |i: usize| {
        if lr {
            graph.nodes[i].height
        } else {
            graph.nodes[i].width
        }
    };
    for cluster in &graph.clusters {
        let members: Vec<usize> = cluster
            .members
            .iter()
            .filter_map(|id| graph.node_index(id))
            .collect();
        if members.is_empty() {
            continue;
        }
        let ranks: Vec<i32> = members.iter().map(|&m| ordered.vnodes[m].rank).collect();
        let (rmin, rmax) = (
            ranks.iter().copied().min().unwrap_or(0),
            ranks.iter().copied().max().unwrap_or(0),
        );
        let lo = members
            .iter()
            .map(|&m| minor(center[m]) - size(m) / 2.0)
            .fold(f64::INFINITY, f64::min);
        let hi = members
            .iter()
            .map(|&m| minor(center[m]) + size(m) / 2.0)
            .fold(f64::NEG_INFINITY, f64::max);

        // `j` indexes nodes across `vnodes`/`center`/`graph.nodes`, not a cursor.
        #[allow(clippy::needless_range_loop)]
        for j in 0..n {
            if members.contains(&j) {
                continue;
            }
            let rj = ordered.vnodes[j].rank;
            if rj < rmin || rj > rmax {
                continue; // above or below the band — already outside the frame
            }
            let m = minor(center[j]);
            if m + size(j) / 2.0 <= lo || m - size(j) / 2.0 >= hi {
                continue; // already clear of the span
            }
            let new_minor = if hi - m <= m - lo {
                hi + graph.nodesep + size(j) / 2.0
            } else {
                lo - graph.nodesep - size(j) / 2.0
            };
            if lr {
                center[j].y = new_minor;
            } else {
                center[j].x = new_minor;
            }
        }
    }
}

/// The bounding box of each cluster: its members' rectangles grown by the
/// cluster margin. Empty members yield no box.
fn cluster_boxes(graph: &Graph, nodes: &[NodePos]) -> Vec<ClusterBox> {
    graph
        .clusters
        .iter()
        .filter_map(|c| {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for m in &c.members {
                let Some(np) = nodes.iter().find(|n| &n.id == m) else {
                    continue;
                };
                min_x = min_x.min(np.center.x - np.width / 2.0);
                min_y = min_y.min(np.center.y - np.height / 2.0);
                max_x = max_x.max(np.center.x + np.width / 2.0);
                max_y = max_y.max(np.center.y + np.height / 2.0);
            }
            if !min_x.is_finite() {
                return None;
            }
            Some(ClusterBox {
                id: c.id.clone(),
                bbox: Box2::new(
                    min_x - c.margin,
                    min_y - c.margin,
                    (max_x - min_x) + 2.0 * c.margin,
                    (max_y - min_y) + 2.0 * c.margin,
                ),
            })
        })
        .collect()
}

/// Translate the assembled layout so its true minimum corner is the origin, and
/// report the tight bounding box over every node rectangle, edge point, and
/// cluster box. (Node centres minus half-widths, and cluster margins, can be
/// negative before this shift.)
fn finish(
    mut nodes: Vec<NodePos>,
    mut edges: Vec<EdgeRoute>,
    mut clusters: Vec<ClusterBox>,
) -> Layout {
    let mut acc = Extent::new();
    for n in &nodes {
        acc.include(n.center.x - n.width / 2.0, n.center.y - n.height / 2.0);
        acc.include(n.center.x + n.width / 2.0, n.center.y + n.height / 2.0);
    }
    for e in &edges {
        for p in e.spline.iter().chain(&e.polyline) {
            acc.include(p.x, p.y);
        }
        if let Some(l) = e.label_pos {
            acc.include(l.x, l.y);
        }
    }
    for c in &clusters {
        acc.include(c.bbox.x, c.bbox.y);
        acc.include(c.bbox.x + c.bbox.w, c.bbox.y + c.bbox.h);
    }

    let Some((min_x, min_y, max_x, max_y)) = acc.bounds() else {
        return Layout::default();
    };
    let (dx, dy) = (-min_x, -min_y);
    for n in &mut nodes {
        n.center.x += dx;
        n.center.y += dy;
    }
    for e in &mut edges {
        for p in e.spline.iter_mut().chain(&mut e.polyline) {
            p.x += dx;
            p.y += dy;
        }
        if let Some(l) = &mut e.label_pos {
            l.x += dx;
            l.y += dy;
        }
    }
    for c in &mut clusters {
        c.bbox.x += dx;
        c.bbox.y += dy;
    }

    Layout {
        bbox: Box2::new(0.0, 0.0, max_x - min_x, max_y - min_y),
        nodes,
        edges,
        clusters,
    }
}

/// A running min/max accumulator over points.
struct Extent {
    bounds: Option<(f64, f64, f64, f64)>, // (min_x, min_y, max_x, max_y)
}

impl Extent {
    fn new() -> Self {
        Self { bounds: None }
    }

    fn include(&mut self, x: f64, y: f64) {
        self.bounds = Some(match self.bounds {
            None => (x, y, x, y),
            Some((nx, ny, mx, my)) => (nx.min(x), ny.min(y), mx.max(x), my.max(y)),
        });
    }

    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        self.bounds
    }
}

/// The midpoint of a polyline by arc-length — where an edge label sits.
fn midpoint(poly: &[Pt]) -> Pt {
    if poly.is_empty() {
        return Pt::new(0.0, 0.0);
    }
    poly[poly.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_lays_out_empty() {
        assert_eq!(layout(&Graph::new()), Layout::default());
    }

    #[test]
    fn places_nodes_and_is_deterministic() {
        let mut g = Graph::new();
        g.nodes.push(Node::new("a", 60.0, 30.0));
        g.nodes.push(Node::new("b", 60.0, 30.0));
        g.edges.push(Edge::new("a", "b"));
        let first = layout(&g);
        assert_eq!(first.nodes.len(), 2);
        assert_eq!(first.edges.len(), 1);
        assert_eq!(layout(&g), first, "layout is deterministic");
    }

    #[test]
    fn bbox_encloses_every_node_edge_and_cluster() {
        // Wide nodes (> 2·nodesep) and a margined cluster: all geometry must sit
        // within bbox at non-negative coordinates after the origin shift.
        let mut g = Graph::new();
        for id in ["a", "b", "c"] {
            g.nodes.push(Node::new(id, 200.0, 40.0));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("b", "c"));
        g.clusters.push(Cluster {
            id: "k".to_owned(),
            members: vec!["b".to_owned()],
            margin: 16.0,
        });
        let l = layout(&g);
        for n in &l.nodes {
            assert!(
                n.center.x - n.width / 2.0 >= -0.01,
                "node left within bbox: {n:?}"
            );
            assert!(n.center.y - n.height / 2.0 >= -0.01, "node top within bbox");
            assert!(
                n.center.x + n.width / 2.0 <= l.bbox.w + 0.01,
                "node right within bbox"
            );
            assert!(
                n.center.y + n.height / 2.0 <= l.bbox.h + 0.01,
                "node bottom within bbox"
            );
        }
        for c in &l.clusters {
            assert!(
                c.bbox.x >= -0.01 && c.bbox.y >= -0.01,
                "cluster within bbox: {c:?}"
            );
            assert!(c.bbox.x + c.bbox.w <= l.bbox.w + 0.01);
            assert!(c.bbox.y + c.bbox.h <= l.bbox.h + 0.01);
        }
        for e in &l.edges {
            for p in &e.polyline {
                assert!(p.x >= -0.01 && p.y >= -0.01, "edge point within bbox");
            }
        }
    }

    #[test]
    fn same_rank_pulls_a_member_onto_a_shared_rank_and_keeps_arrowheads() {
        // a -> b -> c -> d (a chain). Force `d` onto `b`'s rank. Then c -> d points
        // backward (rank c > rank d), but its polyline must still run c -> d so the
        // arrowhead lands on d.
        let mut g = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.nodes.push(Node::new(id, 60.0, 30.0));
        }
        g.edges.push(Edge::new("a", "b"));
        g.edges.push(Edge::new("b", "c"));
        g.edges.push(Edge::new("c", "d"));
        g.same_rank = vec![vec!["b".to_owned(), "d".to_owned()]];

        let l = layout(&g);
        let cy = |id: &str| l.nodes.iter().find(|n| n.id == id).unwrap().center.y;
        assert!((cy("b") - cy("d")).abs() < 0.5, "b and d share a rank");
        assert!(cy("c") > cy("b"), "c is deeper than the b/d rank");

        // The c -> d edge's polyline runs tail(c) -> head(d): first point near c,
        // last near d.
        let cd = l
            .edges
            .iter()
            .find(|e| e.tail == "c" && e.head == "d")
            .unwrap();
        let near = |p: &Pt, id: &str| {
            let n = l.nodes.iter().find(|n| n.id == id).unwrap();
            (p.x - n.center.x).abs() < n.width && (p.y - n.center.y).abs() < n.height
        };
        assert!(
            near(cd.polyline.first().unwrap(), "c"),
            "starts at tail c: {cd:?}"
        );
        assert!(
            near(cd.polyline.last().unwrap(), "d"),
            "ends at head d: {cd:?}"
        );
    }

    #[test]
    fn same_ranked_external_is_kept_outside_the_cluster_frame() {
        // The banking shape: a cluster {m1, m2} and an external `sys` that a
        // same-rank move pulls up onto m1's rank, inside the band. Containment
        // must push `sys` clear of the cluster's x-span so it stays outside the
        // frame.
        let mut g = Graph::new();
        for id in ["top", "m1", "m2", "sys"] {
            g.nodes.push(Node::new(id, 120.0, 40.0));
        }
        g.edges.push(Edge::new("top", "m1"));
        g.edges.push(Edge::new("m1", "m2"));
        g.edges.push(Edge::new("m2", "sys")); // sys is naturally below the band
        g.clusters.push(Cluster {
            id: "c".to_owned(),
            members: vec!["m1".to_owned(), "m2".to_owned()],
            margin: 12.0,
        });
        g.same_rank = vec![vec!["m1".to_owned(), "sys".to_owned()]]; // pull sys into the band

        let l = layout(&g);
        let sys = l.nodes.iter().find(|n| n.id == "sys").unwrap();
        let m1 = l.nodes.iter().find(|n| n.id == "m1").unwrap();
        assert!(
            (sys.center.y - m1.center.y).abs() < 0.5,
            "sys shares m1's rank"
        );
        let frame = l.clusters.iter().find(|c| c.id == "c").unwrap().bbox;
        let left = sys.center.x + sys.width / 2.0 <= frame.x + 0.01;
        let right = sys.center.x - sys.width / 2.0 >= frame.x + frame.w - 0.01;
        assert!(
            left || right,
            "sys is clear of the cluster x-span: sys={sys:?} frame={frame:?}"
        );
    }

    #[test]
    fn cluster_box_frames_members_and_excludes_externals() {
        // The C4 shape: a cluster (system boundary) with two members, a caller
        // above and a callee below. The cluster box must enclose its members and
        // leave both externals outside.
        let mut g = Graph::new();
        for id in ["caller", "m1", "m2", "callee"] {
            g.nodes.push(Node::new(id, 80.0, 40.0));
        }
        g.edges.push(Edge::new("caller", "m1"));
        g.edges.push(Edge::new("m1", "m2"));
        g.edges.push(Edge::new("m2", "callee"));
        g.clusters.push(Cluster {
            id: "boundary".to_owned(),
            members: vec!["m1".to_owned(), "m2".to_owned()],
            margin: 12.0,
        });

        let l = layout(&g);
        let cbox = &l.clusters.iter().find(|c| c.id == "boundary").unwrap().bbox;
        let center = |id: &str| l.nodes.iter().find(|n| n.id == id).unwrap().center;

        let inside = |p: Pt| {
            p.x >= cbox.x && p.x <= cbox.x + cbox.w && p.y >= cbox.y && p.y <= cbox.y + cbox.h
        };
        assert!(
            inside(center("m1")) && inside(center("m2")),
            "members framed"
        );
        assert!(!inside(center("caller")), "caller outside the frame");
        assert!(!inside(center("callee")), "callee outside the frame");
        // The caller sits above the band, the callee below (TB reading order).
        assert!(center("caller").y < cbox.y);
        assert!(center("callee").y > cbox.y + cbox.h);
    }
}
