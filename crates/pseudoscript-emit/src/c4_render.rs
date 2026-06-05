//! C4 view rendering, driven by the [`pseudoscript_dot`] layout engine.
//!
//! [`render_c4`] and [`layout_c4_scene`] turn a [`C4Scene`] into, respectively, a
//! self-contained SVG and a positioned [`C4Layout`] the web IDE's canvas renders
//! directly. Both share one layout pass: the scene becomes a `pseudoscript_dot`
//! [`dot::Graph`] (one box per node sized from its card content, one cluster for
//! a boundary view), `dot::layout` runs the Graphviz `dot` pipeline (rank →
//! order → position → splines, with the boundary as a cluster so external actors
//! land outside its frame), and the result maps back to the [`C4Layout`] DTO.
//!
//! The SVG style is a modern C4 *card*: a white rounded card with a coloured left
//! rule (the kind colour), an UPPERCASE kind eyebrow, a bold title, and a dimmed
//! `///` description wrapped to at most two lines. Edges are the engine's routed
//! Bézier polylines with an arrowhead at the target.
//!
//! A container/component view draws the boundary (`of`) node as an enclosing
//! dashed frame — the cluster box the engine returns — with its label as a title.

use std::collections::HashMap;
use std::fmt::Write as _;

use pseudoscript_dot as dot;
use pseudoscript_model::NodeKind;
use serde::{Deserialize, Serialize};

use crate::render::pal;
use crate::scene::{C4EdgeKind, C4Scene, PlacedNode, Rect, RoutedEdge};

// All SVG colours come from the active theme palette (crate::render::pal); the
// hand-written emitters bind their roles as locals at the top of each function.
/// Padding added around the laid-out extent, inside the document.
const CANVAS_PAD: i32 = 24;
/// Minimum gap between sibling nodes in a rank, handed to the layout engine.
const NODESEP: f64 = 40.0;
/// Minimum gap between ranks, handed to the layout engine (room for edge labels).
const RANKSEP: f64 = 72.0;
/// Padding between a boundary cluster's contents and its frame.
const CLUSTER_MARGIN: f64 = 16.0;
/// Card corner radius.
const CARD_RADIUS: i32 = 8;
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
/// An empty view (only the framed boundary, or no nodes) falls back to the
/// scene's own simple coordinates ([`fallback_svg`]); otherwise the view is laid
/// out by [`layout_c4_scene`] and drawn from that [`C4Layout`].
pub(crate) fn render_c4(scene: &C4Scene) -> String {
    let boundary = scene.of.as_deref();
    let laid_out = scene.nodes.iter().any(|n| Some(n.fqn.as_str()) != boundary);
    if !laid_out {
        return fallback_svg(scene, boundary);
    }
    emit_svg(&layout_c4_scene(scene))
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

/// A routed edge: the engine's Bézier polyline through `points`, plus the
/// relationship it expresses. `points` always has at least two entries. `dashed`
/// marks a `from`-provenance edge, matching the SVG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaidOutEdge {
    /// Source endpoint FQN.
    pub from: String,
    /// Target endpoint FQN.
    pub to: String,
    /// The relationship kind.
    pub kind: C4EdgeKind,
    /// The merged edge labels (call method names), sorted and de-duplicated;
    /// empty for a trigger or provenance edge. The canvas stacks them one per
    /// line, matching the SVG.
    pub labels: Vec<String>,
    /// The routed polyline (at least two points).
    pub points: Vec<PointI>,
    /// The label position (polyline midpoint) when the edge carries labels.
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
    /// The frame rectangle (the cluster box the engine returned).
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

/// Per-diagram layout tweaks (the UI's "Layout" toggles): whether to run the
/// long-edge optimiser, the reading direction, and a spacing multiplier.
#[derive(Debug, Clone, Copy)]
pub struct C4Tweaks {
    /// Run the [`dot::optimize::minimize_long_edges`] search.
    pub minimize_long_edges: bool,
    /// Lay out left-to-right instead of top-to-bottom.
    pub left_to_right: bool,
    /// Multiplier on the base node/rank spacing (1.0 = default).
    pub spacing: f64,
}

impl Default for C4Tweaks {
    fn default() -> Self {
        Self {
            minimize_long_edges: false,
            left_to_right: false,
            spacing: 1.0,
        }
    }
}

/// Positions a [`C4Scene`] into a [`C4Layout`] via the [`pseudoscript_dot`]
/// engine, with default tweaks. See [`layout_c4_scene_with`].
#[must_use]
pub fn layout_c4_scene(scene: &C4Scene) -> C4Layout {
    layout_c4_scene_with(scene, &C4Tweaks::default())
}

/// Positions a [`C4Scene`] into a [`C4Layout`] under `tweaks`. An empty view
/// (only the framed boundary) falls back to the scene's own simple coordinates
/// ([`fallback_layout`]); the engine itself never panics.
#[must_use]
pub fn layout_c4_scene_with(scene: &C4Scene, tweaks: &C4Tweaks) -> C4Layout {
    tracing::debug!(
        of = ?scene.of,
        nodes = scene.nodes.len(),
        edges = scene.edges.len(),
        minimize_long_edges = tweaks.minimize_long_edges,
        left_to_right = tweaks.left_to_right,
        "c4 layout_c4_scene"
    );
    let boundary = scene.of.as_deref();
    let laid_out = scene.nodes.iter().any(|n| Some(n.fqn.as_str()) != boundary);
    if !laid_out {
        return fallback_layout(scene, boundary);
    }

    let drawable = drawable_edges(scene, boundary);
    let graph = to_dot_graph(scene, boundary, &drawable, tweaks);
    let positioned = dot::layout(&graph);
    from_dot_layout(&positioned, scene, boundary, &drawable)
}

/// Builds the engine input from a scene: one box per non-boundary node (sized
/// from its card content), one cluster for a boundary view's children, and one
/// edge per drawable relationship. The boundary anchor is not a box — it becomes
/// the cluster frame.
fn to_dot_graph(
    scene: &C4Scene,
    boundary: Option<&str>,
    drawable: &[&RoutedEdge],
    tweaks: &C4Tweaks,
) -> dot::Graph {
    let mut graph = dot::Graph::new();
    graph.rankdir = if tweaks.left_to_right {
        dot::RankDir::LeftRight
    } else {
        dot::RankDir::TopBottom
    };
    // Spacing scales the node and rank gaps. The rank gap is the axis the diagram
    // flows along — vertical in TB, horizontal in LR — and in LR it competes with
    // the wide C4 cards, so the same multiplier reads as a smaller change. Amplify
    // the rank-gap response in LR so roomy/compact visibly move the X spacing.
    let scale = tweaks.spacing.max(0.1);
    graph.nodesep = NODESEP * scale;
    let rank_scale = if tweaks.left_to_right {
        1.0 + (scale - 1.0) * 3.0
    } else {
        scale
    };
    graph.ranksep = (RANKSEP * rank_scale).max(24.0);

    // Feed persons (actors) first. Node order is the engine's cycle-breaking
    // tie-break — the first node in a cycle becomes the DFS root and ranks at the
    // top — so leading with actors reverses the inbound "notify" edges that close
    // a loop (e.g. `Email -> Customer`) and keeps people at the top, the C4
    // reading convention.
    let mut ordered: Vec<&PlacedNode> = scene
        .nodes
        .iter()
        .filter(|n| Some(n.fqn.as_str()) != boundary)
        .collect();
    ordered.sort_by_key(|n| u8::from(n.kind != NodeKind::Person));
    for node in ordered {
        let (w, h) = card_size(node);
        graph.nodes.push(dot::Node::new(node.fqn.clone(), w, h));
    }

    if let Some(of) = boundary {
        let members: Vec<String> = scene
            .nodes
            .iter()
            .filter(|n| n.boundary.as_deref() == Some(of))
            .map(|n| n.fqn.clone())
            .collect();
        if !members.is_empty() {
            graph.clusters.push(dot::Cluster {
                id: of.to_owned(),
                members,
                margin: CLUSTER_MARGIN,
            });
        }
    }

    for edge in drawable {
        let mut e = dot::Edge::new(edge.from.clone(), edge.to.clone());
        // Reserve space for the edge label so the engine makes the edge long
        // enough to hold it (no cramping against the cards).
        e.label = label_size(&edge.labels);
        graph.edges.push(e);
    }

    if tweaks.minimize_long_edges {
        graph.same_rank = dot::optimize::minimize_long_edges(&graph);
    }
    graph
}

/// Maps the engine's [`dot::Layout`] back to a [`C4Layout`]: cards from placed
/// nodes, edges matched to their routed splines by endpoint FQN, and the
/// boundary frame from the cluster box. All coordinates are shifted by
/// [`CANVAS_PAD`] for breathing room inside the document.
fn from_dot_layout(
    positioned: &dot::Layout,
    scene: &C4Scene,
    boundary: Option<&str>,
    drawable: &[&RoutedEdge],
) -> C4Layout {
    let by_fqn: HashMap<&str, &PlacedNode> =
        scene.nodes.iter().map(|n| (n.fqn.as_str(), n)).collect();
    let pad = CANVAS_PAD;
    let off = |p: dot::Pt| PointI {
        x: round(p.x) + pad,
        y: round(p.y) + pad,
    };

    let mut nodes = Vec::new();
    for np in &positioned.nodes {
        let Some(node) = by_fqn.get(np.id.as_str()) else {
            continue;
        };
        let rect = Rect {
            x: round(np.center.x - np.width / 2.0) + pad,
            y: round(np.center.y - np.height / 2.0) + pad,
            w: round(np.width),
            h: round(np.height),
        };
        nodes.push(laid_out_node(node, rect));
    }

    // Route geometry keyed by endpoint pair (parallel edges of differing kinds
    // share a path; that is acceptable — the geometry is the same).
    let mut routes: HashMap<(&str, &str), &dot::EdgeRoute> = HashMap::new();
    for er in &positioned.edges {
        routes
            .entry((er.tail.as_str(), er.head.as_str()))
            .or_insert(er);
    }

    let mut edges = Vec::new();
    for edge in drawable {
        let Some(route) = routes.get(&(edge.from.as_str(), edge.to.as_str())) else {
            continue;
        };
        let points: Vec<PointI> = route.polyline.iter().map(|p| off(*p)).collect();
        if points.len() < 2 {
            continue;
        }
        let label_pos = if edge.labels.is_empty() {
            None
        } else {
            Some(route.label_pos.map_or(points[points.len() / 2], &off))
        };
        edges.push(LaidOutEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind,
            labels: edge.labels.clone(),
            points,
            label_pos,
            dashed: matches!(edge.kind, C4EdgeKind::Provenance),
        });
    }

    let frame = boundary.and_then(|of| {
        let cluster = positioned.clusters.iter().find(|c| c.id == of)?;
        let rect = Rect {
            x: round(cluster.bbox.x) + pad,
            y: round(cluster.bbox.y) + pad,
            w: round(cluster.bbox.w),
            h: round(cluster.bbox.h),
        };
        boundary_frame(scene, of, rect)
    });

    C4Layout {
        width: round(positioned.bbox.w) + 2 * pad,
        height: round(positioned.bbox.h) + 2 * pad,
        nodes,
        edges,
        boundary: frame,
    }
}

/// Every edge to draw: both endpoints are non-boundary nodes present in the
/// scene, and not a self-loop. (Cycle breaking is the engine's job; it draws
/// every relationship, so a cyclic C4 graph keeps all its arrows.)
fn drawable_edges<'a>(scene: &'a C4Scene, boundary: Option<&str>) -> Vec<&'a RoutedEdge> {
    let in_view = |fqn: &str| Some(fqn) != boundary && scene.nodes.iter().any(|n| n.fqn == fqn);
    scene
        .edges
        .iter()
        .filter(|e| e.from != e.to && in_view(&e.from) && in_view(&e.to))
        .collect()
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

/// A fallback mirroring [`fallback_svg`]: each node at its own scene-assigned
/// rect, straight centre-to-centre edges, and the boundary frame from the
/// boundary node's rect. Used for an empty view (no children to lay out).
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
                labels: e.labels.clone(),
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

/// A panic-proof SVG fallback: draws each node card at its scene-assigned rect
/// with a straight edge between centres. Used for an empty view, so `pds doc`
/// never crashes on a model.
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

// --- card content sizing/wrapping -------------------------------------------

/// The card box `(width, height)` for a node, derived from its content *before*
/// layout so the engine reserves the right footprint. Width is the clamped
/// widest of the title, eyebrow, and wrapped description; height grows with the
/// description's line count. A node with no description gets the short
/// [`NO_DESC_H`] card.
fn card_size(node: &PlacedNode) -> (f64, f64) {
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
    (w, h)
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

// --- SVG emission -----------------------------------------------------------

/// Renders a positioned [`C4Layout`] to a self-contained SVG document: the
/// boundary frame (if any), the node cards, then the routed edges with labels.
fn emit_svg(layout: &C4Layout) -> String {
    let mut out = String::new();
    svg_open(&mut out, layout.width, layout.height);

    if let Some(frame) = &layout.boundary {
        #[allow(non_snake_case)]
        let (CARD_BORDER, TITLE_FILL) = (pal().hairline, pal().ink);
        let _ = write!(
            &mut out,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"12\" \
             fill=\"{boundary_fill}\" stroke=\"{CARD_BORDER}\" stroke-dasharray=\"6 5\"/>\
             <text x=\"{tx}\" y=\"{ty}\" font-size=\"13\" font-weight=\"700\" \
             fill=\"{TITLE_FILL}\">{label}</text>",
            x = frame.rect.x,
            y = frame.rect.y,
            w = frame.rect.w,
            h = frame.rect.h,
            boundary_fill = pal().boundary_fill,
            tx = frame.rect.x + 12,
            ty = frame.rect.y + 19,
            label = escape_xml(&frame.title),
        );
    }

    for node in &layout.nodes {
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
    for edge in &layout.edges {
        draw_arrow(&mut out, &edge.points, edge.dashed);
        if let Some(lp) = edge.label_pos {
            draw_edge_label(&mut out, lp, &edge_display(&edge.labels));
        }
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

/// Draws a routed edge as a polyline with an arrowhead at the target.
fn draw_arrow(out: &mut String, points: &[PointI], dashed: bool) {
    if points.len() < 2 {
        return;
    }
    let mut path = String::new();
    for (i, p) in points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(&mut path, "{cmd}{},{} ", p.x, p.y);
    }
    let dash = if dashed {
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

/// The separator between an edge's merged labels in its display string. Joined by
/// [`edge_display`], split back by [`draw_edge_label`]; the web canvas mirrors it
/// (`C4Flow.svelte`). One newline per stacked label.
const LABEL_SEP: &str = "\n";

/// An edge's display label: its merged labels stacked one per line.
fn edge_display(labels: &[String]) -> String {
    labels.join(LABEL_SEP)
}

/// The `(width, height)` of an edge's label plate, matching [`draw_edge_label`]'s
/// geometry, so the layout engine can reserve room for it. `None` for an
/// unlabelled (trigger/provenance) edge.
fn label_size(labels: &[String]) -> Option<(f64, f64)> {
    if labels.is_empty() {
        return None;
    }
    let widest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = f64::from(u32::try_from(widest).unwrap_or(0)) * f64::from(EDGE_CHAR_W)
        + f64::from(EDGE_PLATE_PAD_X);
    let h = f64::from(u32::try_from(labels.len()).unwrap_or(1)) * f64::from(EDGE_LINE_H)
        + f64::from(EDGE_PLATE_PAD);
    Some((w, h))
}

/// The line height (px) of a stacked edge label, sized for the 11.5px label font.
const EDGE_LINE_H: i32 = 14;
/// The vertical padding (px) added to an edge-label plate's text block.
const EDGE_PLATE_PAD: i32 = 2;
/// Approximate width (px) per character of an edge label.
const EDGE_CHAR_W: i32 = 7;
/// Horizontal padding (px) of an edge-label plate. Shared by the label drawing
/// ([`draw_edge_label`]) and the layout reservation ([`label_size`]) so the space
/// reserved matches the plate drawn.
const EDGE_PLATE_PAD_X: i32 = 8;

/// Draws an edge label on a small light plate so it never reads against a routed
/// line. A merged edge carries its labels `\n`-joined; each becomes a stacked
/// `<tspan>` and the plate grows to fit.
fn draw_edge_label(out: &mut String, pos: PointI, text: &str) {
    if text.is_empty() {
        return;
    }
    let lines: Vec<&str> = text.split(LABEL_SEP).collect();
    let widest = lines
        .iter()
        .map(|line| i32::try_from(line.chars().count()).unwrap_or(0))
        .max()
        .unwrap_or(0);
    let n = i32::try_from(lines.len()).unwrap_or(1);

    let lx = pos.x;
    let ly = pos.y;
    let plate_w = widest * EDGE_CHAR_W + EDGE_PLATE_PAD_X;
    let plate_h = n * EDGE_LINE_H + EDGE_PLATE_PAD;
    let top = ly - plate_h / 2;
    // First baseline sits 12px below the plate top — the single-line plate's
    // baseline offset, so a one-line label renders exactly as before.
    let first_baseline = top + 12;

    #[allow(non_snake_case)]
    let DESC_FILL = pal().muted;
    let _ = write!(
        out,
        "<rect x=\"{rx}\" y=\"{top}\" width=\"{plate_w}\" height=\"{plate_h}\" rx=\"4\" \
         fill=\"{plate}\"/>\
         <text x=\"{lx}\" y=\"{first_baseline}\" text-anchor=\"middle\" font-size=\"11.5\" \
         fill=\"{DESC_FILL}\">",
        plate = pal().edge_plate,
        rx = lx - plate_w / 2,
    );
    for (i, line) in lines.iter().enumerate() {
        let dy = if i == 0 { 0 } else { EDGE_LINE_H };
        let _ = write!(
            out,
            "<tspan x=\"{lx}\" dy=\"{dy}\">{text}</tspan>",
            text = escape_xml(line),
        );
    }
    out.push_str("</text>");
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
                labels: vec!["uses".to_owned()],
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

    /// A mutual call (A→B, B→A) must lay out without panicking and stay
    /// deterministic — the engine breaks the cycle internally.
    #[test]
    fn cyclic_graph_does_not_panic() {
        let mut scene = context_scene();
        scene.edges.push(RoutedEdge {
            from: "m::B".to_owned(),
            to: "m::A".to_owned(),
            kind: C4EdgeKind::Call,
            labels: vec!["calls back".to_owned()],
        });
        let svg = render_c4(&scene);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains('A') && svg.contains('B'));
        assert_eq!(render_c4(&scene), render_c4(&scene));
    }

    #[test]
    fn draw_edge_label_stacks_merged_labels_as_tspans() {
        let mut single = String::new();
        draw_edge_label(&mut single, PointI { x: 40, y: 20 }, "getB");
        assert_eq!(single.matches("<tspan").count(), 1, "one label: one tspan");

        let mut merged = String::new();
        draw_edge_label(&mut merged, PointI { x: 40, y: 20 }, "getB\ngetBb");
        assert_eq!(
            merged.matches("<tspan").count(),
            2,
            "two merged labels: two stacked tspans"
        );
        assert!(merged.contains(">getB</tspan>") && merged.contains(">getBb</tspan>"));
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
                labels: vec!["calls".to_owned()],
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
    fn lr_spacing_has_strong_x_impact() {
        // In left-to-right, spacing must visibly move the X (rank) axis: roomy
        // should be markedly wider than compact.
        let scene = context_scene(); // m::A -> m::B (two ranks)
        let lr = |spacing: f64| {
            layout_c4_scene_with(
                &scene,
                &C4Tweaks {
                    minimize_long_edges: false,
                    left_to_right: true,
                    spacing,
                },
            )
            .width
        };
        let compact = lr(0.7);
        let roomy = lr(1.4);
        assert!(
            roomy > compact + 80,
            "roomy LR widens X over compact: {compact} -> {roomy}"
        );
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
        assert_eq!(edge.labels, ["uses"]);
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
            labels: vec!["back".to_owned()],
        });
        let layout = layout_c4_scene(&scene);
        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(layout_c4_scene(&scene), layout);
    }

    #[test]
    fn layout_c4_scene_draws_every_edge_in_a_cycle() {
        // Cycle-breaking is for *layering* only. Both directions of a mutual
        // relationship must still be drawn, or a cyclic C4 graph silently loses
        // arrows (e.g. a `person -> container` edge that closes a loop).
        let mut scene = context_scene(); // A -> B
        scene.edges.push(RoutedEdge {
            from: "m::B".to_owned(),
            to: "m::A".to_owned(),
            kind: C4EdgeKind::Call,
            labels: vec!["back".to_owned()],
        });
        let layout = layout_c4_scene(&scene);
        assert!(
            layout
                .edges
                .iter()
                .any(|e| e.from == "m::A" && e.to == "m::B"),
            "forward edge drawn: {:?}",
            layout.edges
        );
        assert!(
            layout
                .edges
                .iter()
                .any(|e| e.from == "m::B" && e.to == "m::A"),
            "back edge drawn: {:?}",
            layout.edges
        );
    }

    #[test]
    fn render_c4_draws_a_back_edge() {
        // The SVG path (`pds doc` / `pds svg`) must also draw a cycle-closing
        // back-edge, not just the routed acyclic subset.
        let mut scene = context_scene(); // A -> B "uses"
        scene.edges.push(RoutedEdge {
            from: "m::B".to_owned(),
            to: "m::A".to_owned(),
            kind: C4EdgeKind::Call,
            labels: vec!["back".to_owned()],
        });
        let svg = render_c4(&scene);
        assert!(svg.contains(">uses</tspan>"), "forward edge label drawn");
        assert!(
            svg.contains(">back</tspan>"),
            "back-edge label drawn (a cyclic graph keeps every arrow)"
        );
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
