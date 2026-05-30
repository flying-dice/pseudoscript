//! SVG layout and rendering.
//!
//! The emit crate owns its layout: [`layout_c4`] and [`layout_sequence`] assign
//! deterministic coordinates to a [`Scene`]'s geometry, and [`render_svg`] turns
//! a laid-out scene into a self-contained SVG document with `std::fmt::Write`
//! string-building. No template engine, headless browser, threads, filesystem,
//! or clock — WASM-clean. The output is correct, deterministic, and readable;
//! not pretty.

use std::fmt::Write as _;

use crate::c4_render::render_c4;
use crate::scene::{C4Scene, MessageKind, Rect, Scene, SeqItem, SequenceScene};

// Layout constants (renderer coordinates).
const PAD: i32 = 20;
const NODE_W: i32 = 160;
const NODE_H: i32 = 60;
const NODE_GAP: i32 = 30;
const BOUNDARY_PAD: i32 = 30;
const LIFELINE_GAP: i32 = 200;
const LIFELINE_TOP: i32 = 64;
const MSG_GAP: i32 = 54;
const FRAME_PAD: i32 = 18;
// Sequence-diagram detail.
const SELF_EXTRA: i32 = 26; // extra height a self-message loop needs
const FRAME_HEAD: i32 = 30; // height of a combined-fragment's operator tab band
const FRAME_FOOT: i32 = 14; // padding below a frame's last row
const ACT_W: i32 = 10; // execution-activation bar width
const RET_STUB: i32 = 46; // length of a return arrow to the (implicit) caller

// --- C4 layout --------------------------------------------------------------

/// Assigns rectangles to a C4 scene: boundary nodes get a wide enclosing box,
/// free nodes flow in a row beneath. Deterministic, declaration order.
pub(crate) fn layout_c4(scene: &mut C4Scene) {
    // Free (top-level) nodes flow left-to-right in a row; boundary children flow
    // in a row inside their boundary box.
    let boundary_fqn = scene.of.clone();

    // Position the boundary anchor box first (if any), sized to hold its
    // children, then place children inside it. Free nodes flow below.
    let mut x = PAD;
    let row_y = PAD;

    // Children of the boundary.
    let child_indices: Vec<usize> = scene
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.boundary.is_some())
        .map(|(i, _)| i)
        .collect();

    if let Some(boundary) = &boundary_fqn {
        // Lay children inside the boundary box.
        let inner_x = PAD + BOUNDARY_PAD;
        let inner_y = row_y + BOUNDARY_PAD + NODE_H; // leave a band for the title
        let mut cx = inner_x;
        for &i in &child_indices {
            scene.nodes[i].rect = Rect {
                x: cx,
                y: inner_y,
                w: NODE_W,
                h: NODE_H,
            };
            cx += NODE_W + NODE_GAP;
        }
        let box_w = (cx - NODE_GAP - PAD).max(NODE_W + 2 * BOUNDARY_PAD);
        let box_h = NODE_H + BOUNDARY_PAD * 2 + NODE_H;
        // The anchor node is the boundary box itself.
        for node in &mut scene.nodes {
            if node.fqn == *boundary {
                node.rect = Rect {
                    x: PAD,
                    y: row_y,
                    w: box_w,
                    h: box_h,
                };
            }
        }
    } else {
        // Context view: every node flows in one row.
        for node in &mut scene.nodes {
            node.rect = Rect {
                x,
                y: row_y,
                w: NODE_W,
                h: NODE_H,
            };
            x += NODE_W + NODE_GAP;
        }
    }
}

// --- sequence layout --------------------------------------------------------

/// Assigns lifeline x-positions across the x-axis (first-appearance order).
pub(crate) fn layout_sequence(scene: &mut SequenceScene) {
    for (i, lifeline) in scene.participants.iter_mut().enumerate() {
        lifeline.x = PAD + NODE_W / 2 + i32::try_from(i).unwrap_or(0) * LIFELINE_GAP;
    }
}

/// The width/height a laid-out sequence scene needs. Height is measured by the
/// same row-advancement rules the renderer uses, so nothing clips.
fn sequence_extent(scene: &SequenceScene) -> (i32, i32) {
    let max_x = scene
        .participants
        .iter()
        .map(|l| l.x + NODE_W / 2)
        .max()
        .unwrap_or(NODE_W)
        // room for a self-loop / return stub on the rightmost lifeline
        + RET_STUB
        + PAD;
    let body_top = LIFELINE_TOP + NODE_H + MSG_GAP;
    let height = body_top + content_height(&scene.items) + PAD;
    (max_x, height)
}

/// The vertical space a sequence body occupies, mirroring [`draw_seq_items`]'s
/// advancement: a call/return is one row, a self-message is taller, and a frame
/// adds its operator-tab band and footer padding around its body.
fn content_height(items: &[SeqItem]) -> i32 {
    items
        .iter()
        .map(|item| match item {
            SeqItem::Message(msg) => match msg.kind {
                MessageKind::SelfMsg => MSG_GAP + SELF_EXTRA,
                _ => MSG_GAP,
            },
            SeqItem::Frame(frame) => FRAME_HEAD + content_height(&frame.body) + FRAME_FOOT,
        })
        .sum()
}

// --- SVG rendering ----------------------------------------------------------

/// Renders a laid-out [`Scene`] to a self-contained SVG document.
#[must_use]
pub fn render_svg(scene: &Scene) -> String {
    match scene {
        Scene::C4(c4) => render_c4(c4),
        Scene::Sequence(seq) => render_sequence(seq),
    }
}

/// SVG document header with a viewBox and an arrowhead marker.
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
    out.push_str(
        "<defs><marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"3\" \
         orient=\"auto\" markerUnits=\"strokeWidth\"><path d=\"M0,0 L9,3 L0,6 z\" \
         fill=\"#333\"/></marker></defs>",
    );
}

// Sequence-diagram palette (ink-on-paper, readable on the doc site's light
// plate in either theme).
const SEQ_INK: &str = "#2a2f3a";
const SEQ_LINE: &str = "#c3c8d2";
const SEQ_MUTED: &str = "#6b7280";
const SEQ_ACCENT: &str = "#e8431f";
const SEQ_OK: &str = "#0f9d8a";
const SEQ_ERR: &str = "#d6432a";
const SEQ_FRAME: &str = "#aab0bd";

fn render_sequence(scene: &SequenceScene) -> String {
    let (w, h) = sequence_extent(scene);
    let body_top = LIFELINE_TOP + NODE_H + MSG_GAP;
    let body_bottom = h - PAD;
    let mut out = String::new();
    svg_open(&mut out, w, h);

    // Sequence-specific markers: a solid filled head for calls, a slim open head
    // for returns.
    let _ = write!(
        &mut out,
        "<defs>\
         <marker id=\"seqcall\" markerWidth=\"11\" markerHeight=\"11\" refX=\"9\" refY=\"3.5\" \
         orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0,0 L10,3.5 L0,7 z\" \
         fill=\"{SEQ_INK}\"/></marker>\
         <marker id=\"seqret\" markerWidth=\"12\" markerHeight=\"12\" refX=\"9\" refY=\"4\" \
         orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0,0 L10,4 M0,8 L10,4\" \
         fill=\"none\" stroke=\"{SEQ_MUTED}\" stroke-width=\"1.3\"/></marker></defs>",
    );

    // Eyebrow + the entry callable's name in the band above the lifelines.
    let _ = write!(
        &mut out,
        "<text x=\"{PAD}\" y=\"22\" font-size=\"10\" letter-spacing=\"2\" \
         fill=\"{SEQ_MUTED}\">SEQUENCE</text>\
         <text x=\"{PAD}\" y=\"44\" font-size=\"17\" font-weight=\"700\" fill=\"{SEQ_INK}\">{name}</text>",
        name = escape_xml(simple_name(&scene.entry)),
    );

    // Owner execution-activation bar runs the length of the body: the entry is
    // active throughout its own trace.
    if let Some(owner) = scene.participants.first() {
        let _ = write!(
            &mut out,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{ACT_W}\" height=\"{hgt}\" rx=\"2\" \
             fill=\"{SEQ_ACCENT}\" fill-opacity=\"0.10\" stroke=\"{SEQ_ACCENT}\" stroke-opacity=\"0.5\"/>",
            x = owner.x - ACT_W / 2,
            y = body_top - MSG_GAP / 2,
            hgt = (body_bottom - (body_top - MSG_GAP / 2)).max(0),
        );
    }

    // Lifeline heads (the kind-coloured C4 card) plus the dashed lifeline.
    for lifeline in &scene.participants {
        let x = lifeline.x;
        crate::c4_render::draw_card(
            &mut out,
            x - NODE_W / 2,
            LIFELINE_TOP,
            NODE_W,
            NODE_H,
            lifeline.kind,
            simple_name(&lifeline.fqn),
            None,
        );
        let _ = write!(
            &mut out,
            "<line x1=\"{x}\" y1=\"{ll_top}\" x2=\"{x}\" y2=\"{ll_bot}\" stroke=\"{SEQ_LINE}\" \
             stroke-dasharray=\"2 4\"/>",
            ll_top = LIFELINE_TOP + NODE_H,
            ll_bot = body_bottom,
        );
    }

    let mut y = body_top;
    let mut step = 0;
    draw_seq_items(&mut out, scene, &scene.items, &mut y, &mut step);

    out.push_str("</svg>");
    out
}

/// Draws a sequence body, advancing `y` per row and numbering each call. Frames
/// draw an enclosing combined-fragment around their body.
fn draw_seq_items(
    out: &mut String,
    scene: &SequenceScene,
    items: &[SeqItem],
    y: &mut i32,
    step: &mut u32,
) {
    for item in items {
        match item {
            SeqItem::Message(msg) => match msg.kind {
                MessageKind::Call => {
                    *step += 1;
                    draw_call(out, scene, msg, *y, *step);
                    *y += MSG_GAP;
                }
                MessageKind::SelfMsg => {
                    *step += 1;
                    draw_self(out, scene, msg, *y, *step);
                    *y += MSG_GAP + SELF_EXTRA;
                }
                MessageKind::Return => {
                    draw_return(out, scene, msg, *y);
                    *y += MSG_GAP;
                }
            },
            SeqItem::Frame(frame) => {
                let top = *y;
                *y += FRAME_HEAD;
                draw_seq_items(out, scene, &frame.body, y, step);
                *y += FRAME_FOOT;
                draw_frame(out, scene, frame.kind, &frame.cond, top, *y);
            }
        }
    }
}

/// A call: solid arrow owner → target, numbered, with a short activation bar on
/// the target.
fn draw_call(
    out: &mut String,
    scene: &SequenceScene,
    msg: &crate::scene::Message,
    y: i32,
    step: u32,
) {
    let Some(from_x) = lifeline_x(scene, &msg.from) else {
        return;
    };
    let to_x = lifeline_x(scene, &msg.to).unwrap_or(from_x);
    let dir = if to_x >= from_x { 1 } else { -1 };
    let start = from_x + dir * (ACT_W / 2);
    let end = to_x - dir * (ACT_W / 2);
    // target activation stub
    let _ = write!(
        out,
        "<rect x=\"{ax}\" y=\"{ay}\" width=\"{ACT_W}\" height=\"{ah}\" rx=\"2\" fill=\"#fff\" \
         stroke=\"{SEQ_LINE}\"/>",
        ax = to_x - ACT_W / 2,
        ay = y - 6,
        ah = MSG_GAP - 14,
    );
    let _ = write!(
        out,
        "<line x1=\"{start}\" y1=\"{y}\" x2=\"{end}\" y2=\"{y}\" stroke=\"{SEQ_INK}\" \
         stroke-width=\"1.4\" marker-end=\"url(#seqcall)\"/>",
    );
    step_badge(out, from_x, y, step);
    seq_label(
        out,
        i32::midpoint(from_x, to_x),
        y - 9,
        &msg.label,
        "middle",
        SEQ_INK,
        true,
    );
}

/// A self-message: a rounded loop on the owner's lifeline.
fn draw_self(
    out: &mut String,
    scene: &SequenceScene,
    msg: &crate::scene::Message,
    y: i32,
    step: u32,
) {
    let Some(x) = lifeline_x(scene, &msg.from) else {
        return;
    };
    let lx = x + ACT_W / 2;
    let _ = write!(
        out,
        "<path d=\"M{lx},{y} h34 a6 6 0 0 1 6 6 v8 a6 6 0 0 1 -6 6 h-34\" fill=\"none\" \
         stroke=\"{SEQ_INK}\" stroke-width=\"1.4\" marker-end=\"url(#seqcall)\"/>",
    );
    step_badge(out, x, y, step);
    seq_label(out, lx + 46, y + 4, &msg.label, "start", SEQ_INK, false);
}

/// A return to the (implicit) caller: a dashed arrow off the owner's lifeline,
/// labelled and coloured by its marker (`Ok`/`Some` vs `Err`/`None`).
fn draw_return(out: &mut String, scene: &SequenceScene, msg: &crate::scene::Message, y: i32) {
    let Some(x) = lifeline_x(scene, &msg.from) else {
        return;
    };
    let (colour, text) = return_style(&msg.label);
    let _ = write!(
        out,
        "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"{colour}\" \
         stroke-width=\"1.3\" stroke-dasharray=\"5 3\" marker-end=\"url(#seqret)\"/>",
        x1 = x - ACT_W / 2,
        x2 = x - RET_STUB,
    );
    seq_label(out, x - RET_STUB, y - 9, &text, "start", colour, false);
}

/// A combined fragment (`alt`/`loop`): a framed box with a notched operator tab
/// and the guard.
fn draw_frame(
    out: &mut String,
    scene: &SequenceScene,
    kind: crate::scene::FrameKind,
    cond: &str,
    top: i32,
    bottom: i32,
) {
    let min_x = scene.participants.iter().map(|l| l.x).min().unwrap_or(PAD) - FRAME_PAD;
    let max_x = scene.participants.iter().map(|l| l.x).max().unwrap_or(PAD) + FRAME_PAD + RET_STUB;
    let op = kind.keyword();
    let tab_w = i32::try_from(op.len()).unwrap_or(3) * 8 + 18;
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{top}\" width=\"{w}\" height=\"{h}\" rx=\"4\" fill=\"{SEQ_INK}\" \
         fill-opacity=\"0.015\" stroke=\"{SEQ_FRAME}\"/>\
         <path d=\"M{x},{top} h{tab_w} l-8,16 h-{tab_inner} z\" fill=\"{SEQ_FRAME}\" \
         fill-opacity=\"0.5\" stroke=\"{SEQ_FRAME}\"/>\
         <text x=\"{ox}\" y=\"{oy}\" font-size=\"11\" font-weight=\"700\" fill=\"{SEQ_INK}\">{op}</text>\
         <text x=\"{gx}\" y=\"{oy}\" font-size=\"11\" fill=\"{SEQ_MUTED}\">[{cond}]</text>",
        x = min_x,
        w = max_x - min_x,
        h = bottom - top,
        tab_inner = tab_w - 8,
        ox = min_x + 8,
        oy = top + 15,
        gx = min_x + tab_w + 8,
        cond = escape_xml(cond),
    );
}

/// A small numbered badge at a message's origin, in reading order.
fn step_badge(out: &mut String, x: i32, y: i32, step: u32) {
    let _ = write!(
        out,
        "<circle cx=\"{cx}\" cy=\"{y}\" r=\"8\" fill=\"{SEQ_ACCENT}\"/>\
         <text x=\"{cx}\" y=\"{ty}\" text-anchor=\"middle\" font-size=\"10\" font-weight=\"700\" \
         fill=\"#fff\">{step}</text>",
        cx = x - ACT_W / 2 - 12,
        ty = y + 3,
    );
}

/// A message label with a faint backing pill for legibility over lifelines and
/// activation bars.
fn seq_label(out: &mut String, x: i32, y: i32, text: &str, anchor: &str, colour: &str, pill: bool) {
    let label = escape_xml(text);
    if pill && !text.is_empty() {
        let w = i32::try_from(text.chars().count()).unwrap_or(0) * 7 + 12;
        let _ = write!(
            out,
            "<rect x=\"{rx}\" y=\"{ry}\" width=\"{w}\" height=\"17\" rx=\"4\" fill=\"#fff\" \
             fill-opacity=\"0.92\"/>",
            rx = x - w / 2,
            ry = y - 12,
        );
    }
    let _ = write!(
        out,
        "<text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" font-size=\"12.5\" fill=\"{colour}\">{label}</text>",
    );
}

/// The colour and display text for a return marker.
fn return_style(marker: &str) -> (&'static str, String) {
    match marker {
        "Ok" | "Some" => (SEQ_OK, format!("\u{21a9} {marker}")),
        "Err" | "None" => (SEQ_ERR, format!("\u{21a9} {marker}")),
        "" => (SEQ_MUTED, "\u{21a9} return".to_owned()),
        other => (SEQ_MUTED, format!("\u{21a9} {other}")),
    }
}

// --- helpers ----------------------------------------------------------------

fn lifeline_x(scene: &SequenceScene, fqn: &str) -> Option<i32> {
    scene
        .participants
        .iter()
        .find(|l| l.fqn == fqn)
        .map(|l| l.x)
}

/// The simple (final-segment) name of an FQN, for lifeline labels. A
/// synthesised initiator (`event:Foo`, `scheduler`, `client`, `caller`) keeps
/// its whole token — it is not a `::`-qualified node name.
fn simple_name(fqn: &str) -> &str {
    if is_initiator(fqn) {
        return fqn;
    }
    fqn.rsplit("::").next().unwrap_or(fqn)
}

/// Whether an endpoint token is a synthesised trigger initiator rather than a
/// declared node FQN (`event:<FQN>` carries a single `:`; the actor initiators
/// are bare words with no `::`).
fn is_initiator(token: &str) -> bool {
    token.starts_with("event:") || matches!(token, "scheduler" | "client" | "caller")
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

    #[test]
    fn escapes_xml_metacharacters() {
        assert_eq!(escape_xml("a<b&c>\"d\""), "a&lt;b&amp;c&gt;&quot;d&quot;");
    }

    #[test]
    fn simple_name_takes_leaf() {
        assert_eq!(simple_name("a::b::C"), "C");
        assert_eq!(simple_name("event:a::B"), "event:a::B");
    }
}
