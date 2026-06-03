//! Layout-rs-driven rendering of C4 views.
//!
//! [`render_c4`] builds a `layout::topo::layout::VisualGraph` from a [`C4Scene`]
//! — one box per node, one routed edge per [`RoutedEdge`] — runs the Sugiyama
//! layered placer, and captures the laid-out geometry through a custom
//! [`RenderBackend`] ([`Capture`]). The captured rects, polylines, and labels are
//! then re-emitted as a self-contained SVG of modern C4 *cards*: a white rounded
//! card with a coloured left rule (the kind colour), an UPPERCASE kind eyebrow, a
//! bold title, and a dimmed `///` description wrapped to at most two lines.
//!
//! Each node box is created with an empty engine label and its FQN as the box's
//! `properties`, so the engine draws no centred text — the card chrome is drawn
//! here. Only captured rects whose `properties` match a scene node's FQN are
//! cards; the engine's layout connectors (no/non-matching properties) are skipped.
//!
//! A container/component view frames its children: the boundary (`of`) node is
//! excluded from the layout graph; the children and their inter-child edges are
//! laid out, then an enclosing rounded rectangle is drawn around their bounding
//! box with the boundary's label as a title (the C4 "system boundary" look).
//!
//! The placement is left-to-right (`Orientation::LeftToRight`), which reads well
//! for C4 container/component graphs. layout-rs is deterministic and the capture
//! preserves draw order, so the emitted SVG is byte-stable across runs.

use std::collections::HashMap;
use std::fmt::Write as _;

use layout::core::base::Orientation;
use layout::core::color::Color;
use layout::core::format::{ClipHandle, RenderBackend};
use layout::core::geometry::Point;
use layout::core::style::{LineStyleKind, StyleAttr};
use layout::std_shapes::shapes::{Arrow, Element, LineEndKind, ShapeKind};
use layout::topo::layout::VisualGraph;

use std::panic::{AssertUnwindSafe, catch_unwind};

use pseudoscript_model::NodeKind;
use serde::{Deserialize, Serialize};

use crate::render::pal;
use crate::scene::{C4EdgeKind, C4Scene, PlacedNode, Rect, RoutedEdge};

// All SVG colours come from the active theme palette (crate::render::pal); the
// hand-written emitters bind their roles as locals at the top of each function.
// The engine-side box/edge styles below feed the layout engine only — the
// `Capture` backend ignores their colours — so they keep harmless literals.
/// Margin added around the laid-out extent and inside the document.
const MARGIN: f64 = 24.0;
/// Font size handed to the layout engine for edge-label sizing.
const FONT_SIZE: usize = 13;
/// Card corner radius.
const CARD_RADIUS: i32 = 8;
/// Card corner radius as the engine's `usize` rounding.
const CARD_RADIUS_PX: usize = 8;
/// Width of the coloured left rule.
const BAR_WIDTH: i32 = 5;
/// Left text padding past the left rule.
const TEXT_PAD: i32 = 14;
/// Description lines wrap to at most this many lines (overflow gets an ellipsis).
const MAX_DESC_LINES: usize = 2;
/// Approximate pixels per character for the bold title at its font size.
const TITLE_CHAR_W: f64 = 8.0;
/// Approximate pixels per character for the eyebrow at its font size.
const EYEBROW_CHAR_W: f64 = 6.5;
/// Approximate pixels per character for the description at its font size.
const DESC_CHAR_W: f64 = 6.0;
/// Integer form of [`DESC_CHAR_W`] for character-budget arithmetic.
const DESC_CHAR_W_I32: i32 = 6;
/// Card width is clamped to this range.
const CARD_MIN_W: f64 = 180.0;
const CARD_MAX_W: f64 = 300.0;
/// Vertical band holding the eyebrow and title.
const HEAD_BAND: i32 = 52;
/// Vertical advance per description line.
const DESC_LINE_H: i32 = 15;
/// Bottom padding below the last description line.
const BOTTOM_PAD: i32 = 12;
/// Card height when the node has no description.
const NO_DESC_H: i32 = 64;

/// Renders a laid-out C4 view to a self-contained SVG document.
///
/// The layout engine requires a DAG and panics on a cycle, so only an acyclic
/// subset of edges is laid out ([`acyclic_edges`]); the call is additionally
/// wrapped so no input can panic the renderer — on failure (or an empty view) it
/// falls back to the scene's own simple coordinates ([`fallback_svg`]).
pub(crate) fn render_c4(scene: &C4Scene) -> String {
    let boundary = scene.of.as_deref();
    let laid_out = scene.nodes.iter().any(|n| Some(n.fqn.as_str()) != boundary);
    if !laid_out {
        return fallback_svg(scene, boundary);
    }

    let captured = catch_unwind(AssertUnwindSafe(|| layout_capture(scene, boundary)));
    match captured {
        Ok(capture) => emit_svg(&capture, scene, boundary),
        Err(_) => fallback_svg(scene, boundary),
    }
}

// --- C4 layout export -------------------------------------------------------

/// A fully positioned C4 view — absolute renderer coordinates a consumer draws
/// verbatim, the same geometry [`render_c4`] turns into SVG. Serde-serializable
/// so it crosses the wasm boundary unchanged (the web IDE's interactive canvas
/// renders it directly, as `FlowTimeline` renders a positioned sequence
/// `Layout`). Produced by [`layout_c4_scene`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct C4Layout {
    /// Total canvas width.
    pub width: i32,
    /// Total canvas height.
    pub height: i32,
    /// Placed node cards.
    pub nodes: Vec<LaidOutNode>,
    /// Routed edges between cards.
    pub edges: Vec<LaidOutEdge>,
    /// The enclosing frame of a container/component view; `None` for context.
    pub boundary: Option<BoundaryFrame>,
}

/// A node card placed by the layout engine: its content (for the card chrome)
/// and its rectangle (engine placement + content-derived [`card_size`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaidOutNode {
    /// The node's fully-qualified name.
    pub fqn: String,
    /// The node's C4 kind, for the card's accent and eyebrow.
    pub kind: NodeKind,
    /// The display label (simple name).
    pub label: String,
    /// The node's `///` summary, rendered as the card's dimmed description.
    #[serde(default)]
    pub summary: Option<String>,
    /// The card rectangle.
    pub rect: Rect,
}

/// A routed edge: the engine's polyline through `points` (a straight
/// card-centre-to-centre line when the routed path could not be matched), plus
/// the relationship it expresses. `points` always has at least two entries.
/// `dashed` marks a `from`-provenance edge, matching the SVG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaidOutEdge {
    /// Source endpoint FQN.
    pub from: String,
    /// Target endpoint FQN.
    pub to: String,
    /// The relationship kind.
    pub kind: C4EdgeKind,
    /// Edge label (the method name for a call, else empty).
    pub label: String,
    /// The routed polyline (at least two points).
    pub points: Vec<PointI>,
    /// The engine's label position, when a matching label was found.
    #[serde(default)]
    pub label_pos: Option<PointI>,
    /// Dashed (a `from`-provenance edge), matching the SVG.
    pub dashed: bool,
}

/// The enclosing frame of a container/component view: its rectangle and the
/// boundary node's title and kind (for the frame's accent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryFrame {
    /// The boundary node's display label.
    pub title: String,
    /// The boundary node's C4 kind (system for a container view, container for a
    /// component view), for the frame's accent colour.
    pub kind: NodeKind,
    /// The frame rectangle (padded child bounding box).
    pub rect: Rect,
}

/// An integer point on the canvas (a rounded layout coordinate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PointI {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

/// Positions a [`C4Scene`] into a [`C4Layout`] — the same layout-rs Sugiyama
/// pass [`render_c4`] runs, returned as structured geometry instead of SVG.
/// Wrapped exactly like [`render_c4`]: an empty view or a layout-engine panic
/// falls back to the scene's own simple coordinates ([`fallback_layout`]), so
/// no input can panic the caller.
#[must_use]
pub fn layout_c4_scene(scene: &C4Scene) -> C4Layout {
    let boundary = scene.of.as_deref();
    let laid_out = scene.nodes.iter().any(|n| Some(n.fqn.as_str()) != boundary);
    if !laid_out {
        return fallback_layout(scene, boundary);
    }

    match catch_unwind(AssertUnwindSafe(|| layout_capture(scene, boundary))) {
        Ok(capture) => capture_to_layout(&capture, scene, boundary),
        Err(_) => fallback_layout(scene, boundary),
    }
}

/// Reads the captured geometry into a [`C4Layout`]: cards from the rects whose
/// `properties` name a scene node (the SVG card filter), edges from each
/// laid-out scene edge matched to its routed arrow by endpoint proximity
/// (robust to the engine's draw order and to connector arrows), and the
/// boundary frame and extent from the shared [`frame_and_extent`].
fn capture_to_layout(capture: &Capture, scene: &C4Scene, boundary: Option<&str>) -> C4Layout {
    let by_fqn: HashMap<&str, &PlacedNode> =
        scene.nodes.iter().map(|n| (n.fqn.as_str(), n)).collect();
    let (frame, width, height) = frame_and_extent(capture, scene, boundary);

    let mut centre: HashMap<&str, Point> = HashMap::new();
    let mut nodes = Vec::new();
    for rect in &capture.rects {
        let Some((fqn, node)) = rect
            .properties
            .as_deref()
            .and_then(|fqn| Some((fqn, *by_fqn.get(fqn)?)))
        else {
            continue;
        };
        centre.insert(
            fqn,
            Point::new(rect.xy.x + rect.size.x / 2.0, rect.xy.y + rect.size.y / 2.0),
        );
        nodes.push(laid_out_node(node, rect_of(rect.xy, rect.size)));
    }

    let mut used_arrow = vec![false; capture.arrows.len()];
    let mut used_text = vec![false; capture.texts.len()];
    let mut edges = Vec::new();
    for edge in acyclic_edges(scene, boundary) {
        let (Some(&from_c), Some(&to_c)) =
            (centre.get(edge.from.as_str()), centre.get(edge.to.as_str()))
        else {
            continue;
        };
        let (points, dashed, label_pos) =
            match nearest_arrow(&capture.arrows, &used_arrow, from_c, to_c) {
                Some(i) => {
                    used_arrow[i] = true;
                    let arrow = &capture.arrows[i];
                    let label_pos =
                        nearest_label(&capture.texts, &mut used_text, &edge.label, arrow);
                    (
                        arrow.points.iter().map(point_i).collect(),
                        arrow.dashed,
                        label_pos,
                    )
                }
                None => (
                    // No routed arrow matched: a straight line between card centres,
                    // so every edge always carries a drawable polyline.
                    vec![point_i(&from_c), point_i(&to_c)],
                    matches!(edge.kind, C4EdgeKind::Provenance),
                    None,
                ),
            };
        edges.push(LaidOutEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind,
            label: edge.label.clone(),
            points,
            label_pos,
            dashed,
        });
    }

    C4Layout {
        width,
        height,
        nodes,
        edges,
        boundary: frame.and_then(|(min, max, _title)| {
            boundary_frame(scene, boundary?, rect_corners(min, max))
        }),
    }
}

/// The unused arrow whose ends sit closest to `from`/`to` (the source and target
/// card centres), or `None` when none is left. Endpoint proximity, not draw
/// order, so connector arrows and reordering never mismatch an edge.
fn nearest_arrow(arrows: &[CapturedArrow], used: &[bool], from: Point, to: Point) -> Option<usize> {
    arrows
        .iter()
        .enumerate()
        .filter(|(i, a)| !used[*i] && a.points.len() >= 2)
        .map(|(i, a)| {
            let first = a.points[0];
            let last = a.points[a.points.len() - 1];
            (i, dist2(first, from) + dist2(last, to))
        })
        .min_by(|(_, x), (_, y)| x.total_cmp(y))
        .map(|(i, _)| i)
}

/// The unused captured edge label matching `label` nearest the arrow's midpoint,
/// marking it consumed so parallel same-label edges take distinct labels. `None`
/// for an empty label or when none matches.
fn nearest_label(
    texts: &[CapturedText],
    used: &mut [bool],
    label: &str,
    arrow: &CapturedArrow,
) -> Option<PointI> {
    if label.is_empty() {
        return None;
    }
    let mid = arrow_midpoint(arrow);
    let (j, text) = texts
        .iter()
        .enumerate()
        .filter(|(j, t)| !used[*j] && t.text == label)
        .min_by(|(_, a), (_, b)| dist2(a.xy, mid).total_cmp(&dist2(b.xy, mid)))?;
    used[j] = true;
    Some(point_i(&text.xy))
}

/// The arithmetic mean of an arrow's polyline points (its rough centre).
fn arrow_midpoint(arrow: &CapturedArrow) -> Point {
    let n = f64::from(u32::try_from(arrow.points.len().max(1)).unwrap_or(u32::MAX));
    let sum = arrow
        .points
        .iter()
        .fold(Point::zero(), |acc, p| Point::new(acc.x + p.x, acc.y + p.y));
    Point::new(sum.x / n, sum.y / n)
}

/// Squared Euclidean distance (avoids a `sqrt` — only the ordering matters).
fn dist2(a: Point, b: Point) -> f64 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy
}

/// A captured top-left + size as an integer [`Rect`].
fn rect_of(xy: Point, size: Point) -> Rect {
    Rect {
        x: round(xy.x),
        y: round(xy.y),
        w: round(size.x),
        h: round(size.y),
    }
}

/// A min/max corner pair as an integer [`Rect`] (the boundary frame box).
fn rect_corners(min: Point, max: Point) -> Rect {
    Rect {
        x: round(min.x),
        y: round(min.y),
        w: round(max.x - min.x),
        h: round(max.y - min.y),
    }
}

/// A rounded [`PointI`] from a layout [`Point`].
fn point_i(p: &Point) -> PointI {
    PointI {
        x: round(p.x),
        y: round(p.y),
    }
}

/// The centre of an integer [`Rect`].
fn rect_centre(r: &Rect) -> PointI {
    PointI {
        x: r.x + r.w / 2,
        y: r.y + r.h / 2,
    }
}

/// A scene node as a [`LaidOutNode`] at `rect` (the engine placement or, in the
/// fallback, the node's own rect).
fn laid_out_node(node: &PlacedNode, rect: Rect) -> LaidOutNode {
    LaidOutNode {
        fqn: node.fqn.clone(),
        kind: node.kind,
        label: node.label.clone(),
        summary: node.summary.clone(),
        rect,
    }
}

/// The boundary node's [`BoundaryFrame`] at `rect` (its title and kind from the
/// node), or `None` when `of` names no scene node.
fn boundary_frame(scene: &C4Scene, of: &str, rect: Rect) -> Option<BoundaryFrame> {
    let node = scene.nodes.iter().find(|n| n.fqn == of)?;
    Some(BoundaryFrame {
        title: node.label.clone(),
        kind: node.kind,
        rect,
    })
}

/// The two endpoint nodes of an in-view edge, or `None` when the edge touches
/// the framed boundary or an endpoint is absent. Shared by the SVG fallback and
/// the layout fallback so the two agree on which edges a fallback draws.
fn view_edge_endpoints<'a>(
    scene: &'a C4Scene,
    edge: &RoutedEdge,
    boundary: Option<&str>,
) -> Option<(&'a PlacedNode, &'a PlacedNode)> {
    if Some(edge.from.as_str()) == boundary || Some(edge.to.as_str()) == boundary {
        return None;
    }
    let from = scene.nodes.iter().find(|n| n.fqn == edge.from)?;
    let to = scene.nodes.iter().find(|n| n.fqn == edge.to)?;
    Some((from, to))
}

/// A panic-proof fallback mirroring [`fallback_svg`]: each node at its own
/// scene-assigned rect, straight centre-to-centre edges, and the boundary frame
/// from the boundary node's rect. Used for an empty view or a layout panic.
fn fallback_layout(scene: &C4Scene, boundary: Option<&str>) -> C4Layout {
    let pad = 20;
    let extent =
        |f: fn(&Rect) -> i32| scene.nodes.iter().map(|n| f(&n.rect)).max().unwrap_or(0) + pad;
    let width = extent(|r| r.x + r.w).max(pad);
    let height = extent(|r| r.y + r.h).max(pad);

    let nodes = scene
        .nodes
        .iter()
        .filter(|n| Some(n.fqn.as_str()) != boundary)
        .map(|n| laid_out_node(n, n.rect))
        .collect();

    let edges = scene
        .edges
        .iter()
        .filter_map(|e| {
            let (from, to) = view_edge_endpoints(scene, e, boundary)?;
            Some(LaidOutEdge {
                from: e.from.clone(),
                to: e.to.clone(),
                kind: e.kind,
                label: e.label.clone(),
                points: vec![rect_centre(&from.rect), rect_centre(&to.rect)],
                label_pos: None,
                dashed: matches!(e.kind, C4EdgeKind::Provenance),
            })
        })
        .collect();

    let boundary = boundary
        .and_then(|of| boundary_frame(scene, of, scene.nodes.iter().find(|n| n.fqn == of)?.rect));

    C4Layout {
        width,
        height,
        nodes,
        edges,
        boundary,
    }
}

/// Builds the layout graph (boundary framed out, cycles broken) and captures the
/// engine's placement + edge routing. Each node box carries an empty engine
/// label (so the engine draws no centred text) and its FQN as `properties` (so
/// the card chrome can be matched back to the scene node at draw time).
fn layout_capture(scene: &C4Scene, boundary: Option<&str>) -> Capture {
    let mut graph = VisualGraph::new(Orientation::LeftToRight);
    let mut handles = HashMap::new();
    for node in &scene.nodes {
        if Some(node.fqn.as_str()) == boundary {
            continue; // The boundary frames its children; it is not a peer box.
        }
        let look = node_style();
        let size = card_size(node);
        let element = Element::create_with_properties(
            ShapeKind::Box(String::new()),
            look,
            Orientation::LeftToRight,
            size,
            node.fqn.clone(),
        );
        handles.insert(node.fqn.clone(), graph.add_node(element));
    }

    for edge in acyclic_edges(scene, boundary) {
        let (Some(&from), Some(&to)) = (handles.get(&edge.from), handles.get(&edge.to)) else {
            continue;
        };
        graph.add_edge(edge_arrow(edge), from, to);
    }

    let mut capture = Capture::default();
    graph.do_it(false, false, false, &mut capture);
    capture
}

/// A DFS color used to break cycles: absence is white (unvisited).
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    /// On the current DFS stack — an edge into a `Gray` node closes a cycle.
    Gray,
    /// Fully explored.
    Black,
}

/// The edges fed to the layout engine: a back-edge-free (acyclic) subset of the
/// in-view edges, since the engine panics on a cycle. Self-loops and edges
/// touching the framed boundary are dropped. Deterministic — nodes and their
/// out-edges are visited in scene order.
fn acyclic_edges<'a>(scene: &'a C4Scene, boundary: Option<&str>) -> Vec<&'a RoutedEdge> {
    let in_view = |fqn: &str| Some(fqn) != boundary && scene.nodes.iter().any(|n| n.fqn == fqn);
    let mut adjacency: HashMap<&str, Vec<&RoutedEdge>> = HashMap::new();
    for edge in &scene.edges {
        if edge.from != edge.to && in_view(&edge.from) && in_view(&edge.to) {
            adjacency.entry(edge.from.as_str()).or_default().push(edge);
        }
    }

    let mut color: HashMap<&str, Mark> = HashMap::new();
    let mut kept = Vec::new();
    for node in &scene.nodes {
        if Some(node.fqn.as_str()) != boundary && !color.contains_key(node.fqn.as_str()) {
            dfs_keep(node.fqn.as_str(), &adjacency, &mut color, &mut kept);
        }
    }
    kept
}

/// Visits `node`, keeping every out-edge except a back-edge (to a `Gray` node),
/// whose removal is what makes the kept set acyclic.
fn dfs_keep<'a>(
    node: &'a str,
    adjacency: &HashMap<&'a str, Vec<&'a RoutedEdge>>,
    color: &mut HashMap<&'a str, Mark>,
    kept: &mut Vec<&'a RoutedEdge>,
) {
    color.insert(node, Mark::Gray);
    if let Some(edges) = adjacency.get(node) {
        for &edge in edges {
            let to = edge.to.as_str();
            match color.get(to) {
                Some(Mark::Gray) => {} // back-edge: drop to keep the set acyclic
                Some(Mark::Black) => kept.push(edge), // forward/cross edge: safe
                None => {
                    kept.push(edge);
                    dfs_keep(to, adjacency, color, kept);
                }
            }
        }
    }
    color.insert(node, Mark::Black);
}

/// A panic-proof fallback: draws each node card at its scene-assigned rect with a
/// straight edge between centres. Used for an empty view or if the layout engine
/// fails, so `pds doc` never crashes on a model.
fn fallback_svg(scene: &C4Scene, boundary: Option<&str>) -> String {
    let pad = 20;
    let w = scene
        .nodes
        .iter()
        .map(|n| n.rect.x + n.rect.w)
        .max()
        .unwrap_or(0)
        + pad;
    let h = scene
        .nodes
        .iter()
        .map(|n| n.rect.y + n.rect.h)
        .max()
        .unwrap_or(0)
        + pad;

    let mut out = String::new();
    svg_open(&mut out, w.max(pad), h.max(pad));

    #[allow(non_snake_case)]
    let (CARD_BORDER, TITLE_FILL, STROKE) = (pal().hairline, pal().ink, pal().ink);

    for node in &scene.nodes {
        if Some(node.fqn.as_str()) != boundary {
            continue;
        }
        let _ = write!(
            out,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"12\" fill=\"{boundary_fill}\" \
             stroke=\"{CARD_BORDER}\" stroke-dasharray=\"6 5\"/>\
             <text x=\"{tx}\" y=\"{ty}\" font-size=\"13\" font-weight=\"700\" \
             fill=\"{TITLE_FILL}\">{label}</text>",
            boundary_fill = pal().boundary_fill,
            x = node.rect.x,
            y = node.rect.y,
            w = node.rect.w,
            h = node.rect.h,
            tx = node.rect.x + 12,
            ty = node.rect.y + 19,
            label = escape_xml(&node.label),
        );
    }
    for node in &scene.nodes {
        if Some(node.fqn.as_str()) == boundary {
            continue;
        }
        draw_card(
            &mut out,
            node.rect.x,
            node.rect.y,
            node.rect.w,
            node.rect.h,
            node.kind,
            &node.label,
            node.summary.as_deref(),
        );
    }
    for edge in &scene.edges {
        let Some((from, to)) = view_edge_endpoints(scene, edge, boundary) else {
            continue;
        };
        let _ = write!(
            out,
            "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{STROKE}\" \
             marker-end=\"url(#arrow)\"/>",
            x1 = from.rect.x + from.rect.w / 2,
            y1 = from.rect.y + from.rect.h / 2,
            x2 = to.rect.x + to.rect.w / 2,
            y2 = to.rect.y + to.rect.h / 2,
        );
    }

    crate::render::svg_close(&mut out);
    out
}

/// The engine-side box style. The card itself is drawn here, so the engine only
/// needs to size and place a plain rectangle; its fill never reaches the SVG.
fn node_style() -> StyleAttr {
    StyleAttr {
        line_color: web_color("#c3c8d2"),
        line_width: 1,
        fill_color: Some(web_color("#ffffff")),
        rounded: CARD_RADIUS_PX,
        font_size: FONT_SIZE,
    }
}

/// The arrow style for a C4 edge: a thin line with an arrowhead at the target,
/// dashed for provenance, carrying the edge label for the engine to route.
fn edge_arrow(edge: &RoutedEdge) -> Arrow {
    let line_style = if matches!(edge.kind, C4EdgeKind::Provenance) {
        LineStyleKind::Dashed
    } else {
        LineStyleKind::Normal
    };
    let look = StyleAttr {
        line_color: web_color("#2a2f3a"),
        line_width: 1,
        fill_color: None,
        rounded: 0,
        font_size: FONT_SIZE,
    };
    Arrow::new(
        LineEndKind::None,
        LineEndKind::Arrow,
        line_style,
        &edge.label,
        &look,
        &None,
        &None,
    )
}

// --- card content sizing/wrapping -------------------------------------------

/// The card box size for a node, derived from its content *before* layout so the
/// engine reserves the right footprint. Width is the clamped widest of the title,
/// eyebrow, and wrapped description; height grows with the description's line
/// count. A node with no description gets the short [`NO_DESC_H`] card.
fn card_size(node: &PlacedNode) -> Point {
    let inner_w = card_inner_width(node);
    let w =
        (f64::from(inner_w) + f64::from(BAR_WIDTH + TEXT_PAD * 2)).clamp(CARD_MIN_W, CARD_MAX_W);

    let h = match node.summary.as_deref() {
        None => f64::from(NO_DESC_H),
        Some(summary) => {
            let lines = wrap_summary(summary, text_width(w)).len();
            f64::from(HEAD_BAND + i32::try_from(lines).unwrap_or(0) * DESC_LINE_H + BOTTOM_PAD)
        }
    };
    Point::new(w, h)
}

/// The widest content row (title / eyebrow / unwrapped description), in px,
/// before clamping — drives the card width.
fn card_inner_width(node: &PlacedNode) -> i32 {
    let title = char_count_f64(&node.label) * TITLE_CHAR_W;
    let eyebrow = char_count_f64(node.kind.keyword()) * EYEBROW_CHAR_W;
    let desc = node
        .summary
        .as_deref()
        .map_or(0.0, |s| char_count_f64(s) * DESC_CHAR_W);
    title.max(eyebrow).max(desc).round() as i32
}

/// The character count of `text` as an `f64`, via `u32` so the cast is lossless
/// for any realistic label length.
fn char_count_f64(text: &str) -> f64 {
    f64::from(u32::try_from(text.chars().count()).unwrap_or(u32::MAX))
}

/// The text-column width (px) available for description wrapping inside a card of
/// total width `card_w`.
fn text_width(card_w: f64) -> i32 {
    (card_w - f64::from(BAR_WIDTH + TEXT_PAD * 2))
        .max(0.0)
        .round() as i32
}

/// Wraps `summary` into at most [`MAX_DESC_LINES`] lines that each fit `width_px`,
/// appending an ellipsis to the last line when content overflows. Pure and
/// deterministic, so card sizing and card drawing agree on the line count.
fn wrap_summary(summary: &str, width_px: i32) -> Vec<String> {
    // Integer floor-divide avoids a float→int cast; clamped to at least one char
    // so a degenerate width still makes progress.
    let max_chars = usize::try_from((width_px.max(0) / DESC_CHAR_W_I32).max(1)).unwrap_or(1);

    // Greedily wrap every word, then keep only the first `MAX_DESC_LINES` lines.
    let mut all: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in summary.split_whitespace() {
        let fits =
            current.is_empty() || current.chars().count() + 1 + word.chars().count() <= max_chars;
        if fits {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        } else {
            all.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        all.push(current);
    }

    let overflow = all.len() > MAX_DESC_LINES;
    all.truncate(MAX_DESC_LINES);
    if let Some(last) = all.last_mut().filter(|_| overflow) {
        truncate_with_ellipsis(last, max_chars);
    }
    all
}

/// Truncates `line` so it plus a trailing ellipsis fits `max_chars`.
fn truncate_with_ellipsis(line: &mut String, max_chars: usize) {
    let budget = max_chars.saturating_sub(1).max(1);
    if line.chars().count() > budget {
        let kept: String = line.chars().take(budget).collect();
        *line = kept;
    }
    line.push('\u{2026}');
}

/// The per-kind accent colour (left rule + eyebrow), matching the doc-site palette.
fn kind_color(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Person => "#4f72f0",
        NodeKind::System => "#e23b2e",
        NodeKind::Container => "#0f9d8a",
        NodeKind::Component => "#c77f10",
        NodeKind::Data => "#9333ea",
        NodeKind::Callable => "#db2777",
    }
}

fn web_color(hex: &str) -> Color {
    Color::from_name(hex).unwrap_or_else(|| Color::fast("black"))
}

// --- captured geometry ------------------------------------------------------

/// A box captured from the layout engine: top-left, size, and the `properties`
/// string it was created with (a node FQN for a card; absent for a connector).
struct CapturedRect {
    xy: Point,
    size: Point,
    properties: Option<String>,
}

/// A routed edge captured from the layout engine: a polyline through the
/// path points and whether it is dashed.
struct CapturedArrow {
    points: Vec<Point>,
    dashed: bool,
}

/// A label captured from the layout engine, centred at the engine's chosen
/// point. Node boxes carry empty labels, so these are edge labels.
struct CapturedText {
    xy: Point,
    text: String,
}

/// A [`RenderBackend`] that records draw calls instead of writing pixels, so the
/// crate can re-emit them in its own SVG style.
#[derive(Default)]
struct Capture {
    rects: Vec<CapturedRect>,
    arrows: Vec<CapturedArrow>,
    texts: Vec<CapturedText>,
}

impl RenderBackend for Capture {
    fn draw_rect(
        &mut self,
        xy: Point,
        size: Point,
        _look: &StyleAttr,
        properties: Option<String>,
        _clip: Option<ClipHandle>,
    ) {
        self.rects.push(CapturedRect {
            xy,
            size,
            properties,
        });
    }

    fn draw_text(&mut self, xy: Point, text: &str, _look: &StyleAttr) {
        self.texts.push(CapturedText {
            xy,
            text: text.to_owned(),
        });
    }

    fn draw_arrow(
        &mut self,
        path: &[(Point, Point)],
        dashed: bool,
        _head: (bool, bool),
        _look: &StyleAttr,
        _properties: Option<String>,
        _text: &str,
    ) {
        // The path is [(exit, _), (entry, _), ...]; the first point of each pair
        // traces the polyline.
        let points = path.iter().map(|(p, _)| *p).collect();
        self.arrows.push(CapturedArrow { points, dashed });
    }

    fn draw_line(
        &mut self,
        start: Point,
        stop: Point,
        _look: &StyleAttr,
        _properties: Option<String>,
    ) {
        self.arrows.push(CapturedArrow {
            points: vec![start, stop],
            dashed: false,
        });
    }

    fn draw_circle(
        &mut self,
        xy: Point,
        size: Point,
        _look: &StyleAttr,
        _properties: Option<String>,
    ) {
        // Connectors may render as small circles; record them (no properties) so
        // the extent still accounts for them. They are not drawn as cards.
        self.rects.push(CapturedRect {
            xy,
            size,
            properties: None,
        });
    }

    fn create_clip(&mut self, _xy: Point, _size: Point, _rounded_px: usize) -> ClipHandle {
        0
    }
}

// --- SVG emission -----------------------------------------------------------

/// The boundary frame (padded child bbox + title) and the document extent
/// (`w`, `h`) for a captured layout — the geometry both the SVG emitter and the
/// [`C4Layout`] export derive identically, so the two never drift. The extent
/// covers the captured content plus any boundary frame.
fn frame_and_extent<'a>(
    capture: &Capture,
    scene: &'a C4Scene,
    boundary: Option<&str>,
) -> (Option<(Point, Point, &'a str)>, i32, i32) {
    let boundary_frame = boundary.and_then(|of| {
        let title = boundary_title(scene, of)?;
        // Frame only the boundary's own children, never the external actors the
        // view draws alongside them (`boundary: None`).
        let (min, max) = children_bbox(capture, scene, of)?;
        let pad = MARGIN;
        Some((
            Point::new(min.x - pad, min.y - pad),
            Point::new(max.x + pad, max.y + pad),
            title,
        ))
    });

    let (_, mut max) = content_bbox(capture);
    if let Some((_, frame_max, _)) = &boundary_frame {
        max.x = max.x.max(frame_max.x);
        max.y = max.y.max(frame_max.y);
    }
    (boundary_frame, round(max.x + MARGIN), round(max.y + MARGIN))
}

/// Re-emits captured geometry as a self-contained SVG, framing the boundary
/// children when the view has an `of`.
fn emit_svg(capture: &Capture, scene: &C4Scene, boundary: Option<&str>) -> String {
    let by_fqn: HashMap<&str, &PlacedNode> =
        scene.nodes.iter().map(|n| (n.fqn.as_str(), n)).collect();

    let (boundary_frame, w, h) = frame_and_extent(capture, scene, boundary);

    let mut out = String::new();
    svg_open(&mut out, w, h);

    if let Some((frame_min, frame_max, title)) = &boundary_frame {
        #[allow(non_snake_case)]
        let (CARD_BORDER, TITLE_FILL) = (pal().hairline, pal().ink);
        let x = round(frame_min.x);
        let y = round(frame_min.y);
        let fw = round(frame_max.x - frame_min.x);
        let fh = round(frame_max.y - frame_min.y);
        let _ = write!(
            &mut out,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{fw}\" height=\"{fh}\" rx=\"12\" \
             fill=\"{boundary_fill}\" stroke=\"{CARD_BORDER}\" stroke-dasharray=\"6 5\"/>\
             <text x=\"{tx}\" y=\"{ty}\" font-size=\"13\" font-weight=\"700\" \
             fill=\"{TITLE_FILL}\">{label}</text>",
            boundary_fill = pal().boundary_fill,
            tx = x + 12,
            ty = y + 19,
            label = escape_xml(title),
        );
    }

    // Only rects whose properties name a scene node are cards; connectors and any
    // other layout rects are skipped (they only contribute to the extent).
    for rect in &capture.rects {
        let Some(node) = rect
            .properties
            .as_deref()
            .and_then(|fqn| by_fqn.get(fqn).copied())
        else {
            continue;
        };
        draw_card(
            &mut out,
            round(rect.xy.x),
            round(rect.xy.y),
            round(rect.size.x),
            round(rect.size.y),
            node.kind,
            &node.label,
            node.summary.as_deref(),
        );
    }
    for arrow in &capture.arrows {
        draw_arrow(&mut out, arrow);
    }
    for label in &capture.texts {
        draw_edge_label(&mut out, label);
    }

    crate::render::svg_close(&mut out);
    out
}

/// Draws a modern C4 card at `(x, y)` of size `w`×`h`: a coloured left rule, a
/// white interior with rounded right corners, a thin border, an UPPERCASE kind
/// eyebrow, a bold title, and the wrapped, dimmed description. Used by both the
/// layout path and the fallback so the two stay consistent.
// `x`/`y`/`w`/`h` mirror the SVG rect attributes they feed; spelling them out
// would obscure, not clarify, the geometry.
#[allow(clippy::too_many_arguments, clippy::many_single_char_names)]
pub(crate) fn draw_card(
    out: &mut String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    kind: NodeKind,
    title: &str,
    summary: Option<&str>,
) {
    #[allow(non_snake_case)]
    let (CARD_FILL, CARD_BORDER, TITLE_FILL, DESC_FILL) =
        (pal().card_fill, pal().hairline, pal().ink, pal().muted);
    let accent = kind_color(kind);
    let r = CARD_RADIUS;

    // 1. Base rounded rect in the kind colour — its left strip shows as the rule.
    //    Carries the soft drop shadow so the whole card lifts off the canvas.
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{r}\" fill=\"{accent}\" \
         filter=\"url(#cardlift)\"/>",
    );

    // 2. White interior covering everything past the rule, right corners rounded,
    //    left edge square — so only a `BAR_WIDTH` strip of the base shows.
    let ix = x + BAR_WIDTH;
    let right = x + w;
    let bottom = y + h;
    let _ = write!(
        out,
        "<path d=\"M{ix},{y} H{rl} A{r},{r} 0 0 1 {right},{ry} V{by} A{r},{r} 0 0 1 {rl},{bottom} \
         H{ix} Z\" fill=\"{CARD_FILL}\"/>",
        rl = right - r,
        ry = y + r,
        by = bottom - r,
    );

    // 3. Border over the whole card.
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{r}\" fill=\"none\" \
         stroke=\"{CARD_BORDER}\"/>",
    );

    // 4. Text, left-aligned past the rule.
    let tx = x + BAR_WIDTH + TEXT_PAD;
    let _ = write!(
        out,
        "<text x=\"{tx}\" y=\"{ey}\" font-size=\"10\" letter-spacing=\"1.5\" font-weight=\"600\" \
         fill=\"{accent}\">{eyebrow}</text>",
        ey = y + 20,
        eyebrow = escape_xml(&kind.keyword().to_uppercase()),
    );
    let _ = write!(
        out,
        "<text x=\"{tx}\" y=\"{ty}\" font-size=\"15\" font-weight=\"700\" \
         fill=\"{TITLE_FILL}\">{title}</text>",
        ty = y + 40,
        title = escape_xml(title),
    );
    if let Some(summary) = summary {
        for (i, line) in wrap_summary(summary, text_width(f64::from(w)))
            .iter()
            .enumerate()
        {
            let _ = write!(
                out,
                "<text x=\"{tx}\" y=\"{ly}\" font-size=\"11.5\" fill=\"{DESC_FILL}\">{line}</text>",
                ly = y + 58 + i32::try_from(i).unwrap_or(0) * DESC_LINE_H,
                line = escape_xml(line),
            );
        }
    }
}

/// The boundary node's label, if the `of` node is present in the scene.
fn boundary_title<'a>(scene: &'a C4Scene, of: &str) -> Option<&'a str> {
    scene
        .nodes
        .iter()
        .find(|n| n.fqn == of)
        .map(|n| n.label.as_str())
}

/// The bounding box of the rects belonging to `of`'s children (the nodes whose
/// `boundary` is `of`). `None` when no child rect was captured.
fn children_bbox(capture: &Capture, scene: &C4Scene, of: &str) -> Option<(Point, Point)> {
    let children: std::collections::HashSet<&str> = scene
        .nodes
        .iter()
        .filter(|n| n.boundary.as_deref() == Some(of))
        .map(|n| n.fqn.as_str())
        .collect();
    let mut min = Point::new(f64::MAX, f64::MAX);
    let mut max = Point::new(f64::MIN, f64::MIN);
    for rect in &capture.rects {
        if !rect
            .properties
            .as_deref()
            .is_some_and(|p| children.contains(p))
        {
            continue;
        }
        min.x = min.x.min(rect.xy.x);
        min.y = min.y.min(rect.xy.y);
        max.x = max.x.max(rect.xy.x + rect.size.x);
        max.y = max.y.max(rect.xy.y + rect.size.y);
    }
    (min.x <= max.x).then_some((min, max))
}

/// The bounding box of all captured content (rects and arrow points).
fn content_bbox(capture: &Capture) -> (Point, Point) {
    let mut min = Point::new(f64::MAX, f64::MAX);
    let mut max = Point::new(f64::MIN, f64::MIN);
    let mut grow = |p: Point| {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    };
    for rect in &capture.rects {
        grow(rect.xy);
        grow(Point::new(rect.xy.x + rect.size.x, rect.xy.y + rect.size.y));
    }
    for arrow in &capture.arrows {
        for &p in &arrow.points {
            grow(p);
        }
    }
    if min.x > max.x {
        return (Point::zero(), Point::zero());
    }
    (min, max)
}

/// SVG document header with a viewBox and an arrowhead marker (matches the
/// sequence renderer's chrome).
fn svg_open(out: &mut String, w: i32, h: i32) {
    let _ = write!(
        out,
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" ",
            "width=\"{w}\" height=\"{h}\" font-family=\"sans-serif\" font-size=\"13\">",
        ),
        w = w,
        h = h,
    );
    let _ = write!(
        out,
        "<defs>\
         <marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"3\" \
         orient=\"auto\" markerUnits=\"strokeWidth\"><path d=\"M0,0 L9,3 L0,6 z\" \
         fill=\"{ink}\"/></marker>\
         <filter id=\"cardlift\" x=\"-12%\" y=\"-12%\" width=\"124%\" height=\"140%\">\
         <feDropShadow dx=\"0\" dy=\"2\" stdDeviation=\"3.5\" flood-color=\"{shadow}\" \
         flood-opacity=\"0.12\"/></filter></defs>",
        ink = pal().ink,
        shadow = pal().shadow,
    );
    // `<text>` inherits the font from this group even in renderers (e.g. JSVG, in
    // the JetBrains plugin) that don't inherit it from the root `<svg>`.
    out.push_str(crate::render::SVG_FONT_GROUP);
}

/// Draws a captured routed edge as a polyline with an arrowhead at the target.
fn draw_arrow(out: &mut String, arrow: &CapturedArrow) {
    if arrow.points.len() < 2 {
        return;
    }
    let mut path = String::new();
    for (i, p) in arrow.points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(&mut path, "{cmd}{},{} ", round(p.x), round(p.y));
    }
    let dash = if arrow.dashed {
        " stroke-dasharray=\"4 3\""
    } else {
        ""
    };
    #[allow(non_snake_case)]
    let STROKE = pal().ink;
    let _ = write!(
        out,
        "<path d=\"{path}\" fill=\"none\" stroke=\"{STROKE}\"{dash} \
         marker-end=\"url(#arrow)\"/>",
    );
}

/// Draws a captured edge label on a small light plate so it never reads against a
/// routed line. Node boxes carry empty labels, so every captured text is an edge
/// label.
fn draw_edge_label(out: &mut String, label: &CapturedText) {
    if label.text.is_empty() {
        return;
    }
    let lx = round(label.xy.x);
    let ly = round(label.xy.y);
    let chars = i32::try_from(label.text.chars().count()).unwrap_or(0);
    let plate_w = chars * 7 + 8;
    #[allow(non_snake_case)]
    let DESC_FILL = pal().muted;
    let _ = write!(
        out,
        "<rect x=\"{rx}\" y=\"{ry}\" width=\"{plate_w}\" height=\"16\" rx=\"4\" \
         fill=\"{plate}\"/>\
         <text x=\"{lx}\" y=\"{ty}\" text-anchor=\"middle\" font-size=\"11.5\" \
         fill=\"{DESC_FILL}\">{text}</text>",
        plate = pal().edge_plate,
        rx = lx - plate_w / 2,
        ry = ly - 8,
        ty = ly + 4,
        text = escape_xml(&label.text),
    );
}

/// Rounds a layout coordinate to the nearest integer SVG unit.
fn round(v: f64) -> i32 {
    v.round() as i32
}

/// Escapes text for an SVG `<text>` body / attribute.
fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{C4View, Rect, RoutedEdge};

    fn placed(fqn: &str, kind: NodeKind, label: &str, summary: Option<&str>) -> PlacedNode {
        PlacedNode {
            fqn: fqn.to_owned(),
            kind,
            label: label.to_owned(),
            summary: summary.map(str::to_owned),
            boundary: None,
            rect: Rect::default(),
        }
    }

    fn context_scene() -> C4Scene {
        C4Scene {
            view: C4View::Context,
            of: None,
            nodes: vec![
                placed("m::A", NodeKind::Person, "A", None),
                placed("m::B", NodeKind::System, "B", None),
            ],
            edges: vec![RoutedEdge {
                from: "m::A".to_owned(),
                to: "m::B".to_owned(),
                kind: C4EdgeKind::Call,
                label: "uses".to_owned(),
            }],
        }
    }

    #[test]
    fn renders_self_contained_svg() {
        let svg = render_c4(&context_scene());
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains('A'));
        assert!(svg.contains('B'));
        assert!(svg.contains("uses"));
    }

    #[test]
    fn renders_kind_eyebrows() {
        let svg = render_c4(&context_scene());
        assert!(svg.contains("PERSON"), "eyebrow for person: {svg}");
        assert!(svg.contains("SYSTEM"), "eyebrow for system: {svg}");
    }

    #[test]
    fn render_is_deterministic() {
        let scene = context_scene();
        assert_eq!(render_c4(&scene), render_c4(&scene));
    }

    /// A card with a summary renders both the bold title and the (escaped)
    /// description text.
    #[test]
    fn card_renders_title_and_summary() {
        let scene = C4Scene {
            view: C4View::Context,
            of: None,
            nodes: vec![
                placed(
                    "m::Shop",
                    NodeKind::System,
                    "Shop",
                    Some("The retail storefront & checkout"),
                ),
                placed("m::User", NodeKind::Person, "User", None),
            ],
            edges: Vec::new(),
        };
        let svg = render_c4(&scene);
        assert!(svg.contains("Shop"), "title present: {svg}");
        assert!(svg.contains("storefront"), "summary present: {svg}");
        assert!(svg.contains("&amp;"), "summary escaped: {svg}");
    }

    /// Regression: the layout engine requires a DAG and panics on a cycle. A
    /// mutual call (A→B, B→A) must lay out without panicking and stay
    /// deterministic — back-edges are dropped from the layout (`acyclic_edges`).
    #[test]
    fn cyclic_graph_does_not_panic() {
        let mut scene = context_scene();
        scene.edges.push(RoutedEdge {
            from: "m::B".to_owned(),
            to: "m::A".to_owned(),
            kind: C4EdgeKind::Call,
            label: "calls back".to_owned(),
        });
        let svg = render_c4(&scene);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains('A') && svg.contains('B'));
        assert_eq!(render_c4(&scene), render_c4(&scene));
    }

    #[test]
    fn acyclic_edges_drops_one_back_edge() {
        let mut scene = context_scene(); // A→B
        scene.edges.push(RoutedEdge {
            from: "m::B".to_owned(),
            to: "m::A".to_owned(),
            kind: C4EdgeKind::Call,
            label: "back".to_owned(),
        });
        // one of the two mutual edges is dropped, leaving an acyclic set
        assert_eq!(acyclic_edges(&scene, None).len(), 1);
    }

    #[test]
    fn wrap_summary_is_bounded_and_ellipsises() {
        let long = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu";
        let lines = wrap_summary(long, 120);
        assert!(lines.len() <= MAX_DESC_LINES);
        assert!(
            lines.last().unwrap().ends_with('\u{2026}'),
            "overflow ellipsis: {lines:?}"
        );
    }

    #[test]
    fn wrap_summary_short_fits_one_line_no_ellipsis() {
        let lines = wrap_summary("a short note", 300);
        assert_eq!(lines, vec!["a short note".to_owned()]);
    }

    // --- layout_c4_scene --------------------------------------------------

    /// A container view: a system boundary with two component-style children and
    /// a call between them.
    fn container_scene() -> C4Scene {
        let child = |fqn: &str, label: &str| PlacedNode {
            boundary: Some("m::Sys".to_owned()),
            ..placed(fqn, NodeKind::Container, label, None)
        };
        C4Scene {
            view: C4View::Container,
            of: Some("m::Sys".to_owned()),
            nodes: vec![
                placed("m::Sys", NodeKind::System, "Sys", None),
                child("m::Sys::Web", "Web"),
                child("m::Sys::Api", "Api"),
            ],
            edges: vec![RoutedEdge {
                from: "m::Sys::Web".to_owned(),
                to: "m::Sys::Api".to_owned(),
                kind: C4EdgeKind::Call,
                label: "calls".to_owned(),
            }],
        }
    }

    #[test]
    fn layout_c4_scene_positions_every_node() {
        let layout = layout_c4_scene(&context_scene());
        assert_eq!(layout.nodes.len(), 2, "both nodes placed: {layout:?}");
        assert!(layout.width > 0 && layout.height > 0, "canvas sized");
        for node in &layout.nodes {
            assert!(node.rect.w > 0 && node.rect.h > 0, "card sized: {node:?}");
        }
    }

    #[test]
    fn layout_c4_scene_is_deterministic() {
        let scene = context_scene();
        assert_eq!(layout_c4_scene(&scene), layout_c4_scene(&scene));
    }

    #[test]
    fn layout_c4_scene_edges_carry_points_and_kind() {
        let layout = layout_c4_scene(&context_scene());
        let edge = layout.edges.first().expect("the A->B edge is laid out");
        assert_eq!((edge.from.as_str(), edge.to.as_str()), ("m::A", "m::B"));
        assert_eq!(edge.kind, C4EdgeKind::Call);
        assert_eq!(edge.label, "uses");
        assert!(edge.points.len() >= 2, "routed polyline: {edge:?}");
    }

    #[test]
    fn layout_c4_scene_frames_a_container_view() {
        let layout = layout_c4_scene(&container_scene());
        let frame = layout
            .boundary
            .expect("container view has a boundary frame");
        assert_eq!(frame.title, "Sys");
        // The frame encloses the two children, which are the only laid-out cards.
        assert_eq!(layout.nodes.len(), 2, "boundary itself is not a card");
        for node in &layout.nodes {
            assert!(
                node.rect.x >= frame.rect.x,
                "child inside frame x: {node:?}"
            );
            assert!(
                node.rect.y >= frame.rect.y,
                "child inside frame y: {node:?}"
            );
        }
    }

    #[test]
    fn layout_c4_scene_cyclic_graph_no_panic() {
        let mut scene = context_scene();
        scene.edges.push(RoutedEdge {
            from: "m::B".to_owned(),
            to: "m::A".to_owned(),
            kind: C4EdgeKind::Call,
            label: "back".to_owned(),
        });
        let layout = layout_c4_scene(&scene);
        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(layout_c4_scene(&scene), layout);
    }

    /// An empty view (only the framed boundary, no children) falls back without
    /// panicking and produces no cards.
    #[test]
    fn layout_c4_scene_empty_view_falls_back() {
        let scene = C4Scene {
            view: C4View::Container,
            of: Some("m::Sys".to_owned()),
            nodes: vec![placed("m::Sys", NodeKind::System, "Sys", None)],
            edges: Vec::new(),
        };
        let layout = layout_c4_scene(&scene);
        assert!(layout.nodes.is_empty(), "no children to draw");
        assert_eq!(
            layout.boundary.map(|b| b.title),
            Some("Sys".to_owned()),
            "fallback still frames the boundary"
        );
    }
}
