//! Edge routing — `dot`'s spline pass (`lib/common/routespl.c`, `lib/dotgen/
//! dotsplines.c`).
//!
//! Each edge has, from the ordering pass, a *corridor*: the centreline through
//! its virtual nodes, rank by rank. `dot` fits a piecewise Bézier inside the box
//! channel around that corridor; for the layered graphs this engine targets the
//! corridor centreline is the channel centre, so a Catmull-Rom spline through
//! the corridor points — clipped to the endpoint node borders — reproduces the
//! same smooth curve. (The one simplification from full `routespl` is obstacle
//! avoidance inside a box: not needed when nodes reserve their own rank slots.)
//!
//! The result is both the cubic control points (`1 + 3k` for `k` segments,
//! tail → head) and a dense flattened polyline a polyline renderer draws verbatim.

use crate::layout::Pt;

/// Samples taken along each cubic segment when flattening to a polyline.
const FLATTEN_STEPS: usize = 12;

/// A node's placed box: centre and half-extents, for endpoint clipping.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NodeBox {
    pub center: Pt,
    pub half_w: f64,
    pub half_h: f64,
}

/// Route one edge through its corridor `points` (tail → head, centres of the
/// chain nodes), clipping the ends to the tail/head boxes. Returns the cubic
/// control points and the flattened polyline.
pub(crate) fn route_edge(points: &[Pt], tail: NodeBox, head: NodeBox) -> (Vec<Pt>, Vec<Pt>) {
    // Degenerate corridors: fall back to a straight clipped segment.
    if points.len() < 2 {
        let a = tail.center;
        let b = head.center;
        let p0 = clip_to_box(tail, b);
        let p1 = clip_to_box(head, a);
        return (straight_cubic(p0, p1), vec![p0, p1]);
    }

    // Clip the first/last corridor point back to the node borders.
    let mut pts: Vec<Pt> = points.to_vec();
    let first = pts[1];
    pts[0] = clip_to_box(tail, first);
    let last = pts[pts.len() - 2];
    let n = pts.len();
    pts[n - 1] = clip_to_box(head, last);

    let spline = catmull_rom_to_bezier(&pts);
    let polyline = flatten(&spline);
    (spline, polyline)
}

/// The point where the ray from `b.center` toward `target` meets `b`'s border.
fn clip_to_box(b: NodeBox, target: Pt) -> Pt {
    let dx = target.x - b.center.x;
    let dy = target.y - b.center.y;
    if dx == 0.0 && dy == 0.0 {
        return b.center;
    }
    let tx = if dx == 0.0 {
        f64::INFINITY
    } else {
        b.half_w / dx.abs()
    };
    let ty = if dy == 0.0 {
        f64::INFINITY
    } else {
        b.half_h / dy.abs()
    };
    let t = tx.min(ty).min(1.0);
    Pt::new(b.center.x + dx * t, b.center.y + dy * t)
}

/// A single straight cubic between two points (control points at the thirds).
fn straight_cubic(p0: Pt, p1: Pt) -> Vec<Pt> {
    let c1 = lerp(p0, p1, 1.0 / 3.0);
    let c2 = lerp(p0, p1, 2.0 / 3.0);
    vec![p0, c1, c2, p1]
}

/// Convert a polyline through `pts` into a smooth piecewise cubic Bézier using
/// Catmull-Rom tangents. Returns `1 + 3k` control points for `k` segments.
fn catmull_rom_to_bezier(pts: &[Pt]) -> Vec<Pt> {
    if pts.len() == 2 {
        return straight_cubic(pts[0], pts[1]);
    }
    let mut out = vec![pts[0]];
    for i in 0..pts.len() - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        // Catmull-Rom -> Bézier control points (tension 0).
        let c1 = Pt::new(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0);
        let c2 = Pt::new(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0);
        out.push(c1);
        out.push(c2);
        out.push(p2);
    }
    out
}

/// Flatten a piecewise cubic (control points `1 + 3k`) to a dense polyline.
/// `FLATTEN_STEPS` is a tiny constant, so the step-to-`f64` casts are exact.
#[allow(clippy::cast_precision_loss)]
fn flatten(spline: &[Pt]) -> Vec<Pt> {
    if spline.len() < 4 {
        return spline.to_vec();
    }
    let mut out = vec![spline[0]];
    let mut i = 0;
    while i + 3 < spline.len() {
        let (p0, p1, p2, p3) = (spline[i], spline[i + 1], spline[i + 2], spline[i + 3]);
        for s in 1..=FLATTEN_STEPS {
            let t = s as f64 / FLATTEN_STEPS as f64;
            out.push(cubic(p0, p1, p2, p3, t));
        }
        i += 3;
    }
    out
}

/// A point on the cubic Bézier `(p0, p1, p2, p3)` at parameter `t`.
fn cubic(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f64) -> Pt {
    let mt = 1.0 - t;
    let (b0, b1, b2, b3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
    Pt::new(
        b0 * p0.x + b1 * p1.x + b2 * p2.x + b3 * p3.x,
        b0 * p0.y + b1 * p1.y + b2 * p2.y + b3 * p3.y,
    )
}

fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
    Pt::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxx(cx: f64, cy: f64) -> NodeBox {
        NodeBox {
            center: Pt::new(cx, cy),
            half_w: 30.0,
            half_h: 15.0,
        }
    }

    #[test]
    fn straight_edge_starts_and_ends_on_borders() {
        // Vertical edge between two boxes; ends clip to the box borders.
        let (spline, poly) = route_edge(
            &[Pt::new(0.0, 0.0), Pt::new(0.0, 100.0)],
            boxx(0.0, 0.0),
            boxx(0.0, 100.0),
        );
        assert_eq!(spline.len(), 4, "one cubic segment");
        // Tail border at y = +half_h = 15; head border at y = 100 - 15 = 85.
        assert!((poly.first().unwrap().y - 15.0).abs() < 0.01);
        assert!((poly.last().unwrap().y - 85.0).abs() < 0.01);
    }

    #[test]
    fn bent_edge_produces_multiple_cubics() {
        // Three corridor points -> two cubic segments (7 control points).
        let pts = [Pt::new(0.0, 0.0), Pt::new(20.0, 50.0), Pt::new(0.0, 100.0)];
        let (spline, poly) = route_edge(&pts, boxx(0.0, 0.0), boxx(0.0, 100.0));
        assert_eq!(spline.len(), 7);
        assert!(poly.len() > 7, "flattened densely");
    }

    #[test]
    fn flattened_polyline_runs_tail_to_head() {
        let pts = [Pt::new(0.0, 0.0), Pt::new(0.0, 60.0), Pt::new(0.0, 120.0)];
        let (_, poly) = route_edge(&pts, boxx(0.0, 0.0), boxx(0.0, 120.0));
        assert!(poly.first().unwrap().y < poly.last().unwrap().y);
    }

    #[test]
    fn deterministic() {
        let pts = [Pt::new(0.0, 0.0), Pt::new(10.0, 40.0), Pt::new(0.0, 80.0)];
        let a = route_edge(&pts, boxx(0.0, 0.0), boxx(0.0, 80.0));
        let b = route_edge(&pts, boxx(0.0, 0.0), boxx(0.0, 80.0));
        assert_eq!(a, b);
    }
}
