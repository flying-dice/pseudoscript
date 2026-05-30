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
const LIFELINE_GAP: i32 = 180;
const LIFELINE_TOP: i32 = 60;
const MSG_GAP: i32 = 50;
const FRAME_PAD: i32 = 16;

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

/// The width/height a laid-out sequence scene needs.
fn sequence_extent(scene: &SequenceScene) -> (i32, i32) {
    let max_x = scene
        .participants
        .iter()
        .map(|l| l.x + NODE_W / 2)
        .max()
        .unwrap_or(NODE_W)
        + PAD;
    let height = LIFELINE_TOP + NODE_H + (message_count(&scene.items) + 1) * MSG_GAP + PAD;
    (max_x, height)
}

/// The number of message rows a sequence body occupies (frames count their
/// bodies, plus a row for the frame header).
fn message_count(items: &[SeqItem]) -> i32 {
    items
        .iter()
        .map(|item| match item {
            SeqItem::Message(_) => 1,
            SeqItem::Frame(frame) => 1 + message_count(&frame.body),
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

fn render_sequence(scene: &SequenceScene) -> String {
    let (w, h) = sequence_extent(scene);
    let mut out = String::new();
    svg_open(&mut out, w, h);

    // Title: the entry callable's simple name, in the band above the lifelines.
    let _ = write!(
        &mut out,
        "<text x=\"{PAD}\" y=\"28\" font-weight=\"bold\">{}</text>",
        escape_xml(simple_name(&scene.entry)),
    );

    // Lifeline heads (the same modern C4 card as the box views) plus the dashed
    // lifeline dropping from each.
    for lifeline in &scene.participants {
        let x = lifeline.x;
        let head_x = x - NODE_W / 2;
        crate::c4_render::draw_card(
            &mut out,
            head_x,
            LIFELINE_TOP,
            NODE_W,
            NODE_H,
            lifeline.kind,
            simple_name(&lifeline.fqn),
            None,
        );
        let _ = write!(
            &mut out,
            "<line x1=\"{x}\" y1=\"{ll_top}\" x2=\"{x}\" y2=\"{ll_bot}\" stroke=\"#999\" \
             stroke-dasharray=\"3 3\"/>",
            ll_top = LIFELINE_TOP + NODE_H,
            ll_bot = h - PAD,
        );
    }

    let mut y = LIFELINE_TOP + NODE_H + MSG_GAP;
    draw_seq_items(&mut out, scene, &scene.items, &mut y);

    out.push_str("</svg>");
    out
}

/// Draws a sequence body, advancing `y` per row. Frames draw an enclosing
/// rectangle around their body.
fn draw_seq_items(out: &mut String, scene: &SequenceScene, items: &[SeqItem], y: &mut i32) {
    for item in items {
        match item {
            SeqItem::Message(msg) => {
                draw_message(out, scene, msg, *y);
                *y += MSG_GAP;
            }
            SeqItem::Frame(frame) => {
                let top = *y;
                let label_y = *y + 16;
                *y += MSG_GAP;
                draw_seq_items(out, scene, &frame.body, y);
                let bottom = *y;
                draw_frame_box(out, scene, frame.kind, &frame.cond, top, label_y, bottom);
            }
        }
    }
}

fn draw_message(out: &mut String, scene: &SequenceScene, msg: &crate::scene::Message, y: i32) {
    let Some(from_x) = lifeline_x(scene, &msg.from) else {
        return;
    };
    let to_x = lifeline_x(scene, &msg.to).unwrap_or(from_x);
    match msg.kind {
        MessageKind::SelfMsg | MessageKind::Return => {
            // A self/return message loops on the owner's lifeline.
            let _ = write!(
                out,
                "<path d=\"M{x},{y} h30 v16 h-30\" fill=\"none\" stroke=\"#333\" \
                 marker-end=\"url(#arrow)\"/>\
                 <text x=\"{tx}\" y=\"{ty}\">{label}</text>",
                x = from_x,
                tx = from_x + 36,
                ty = y + 4,
                label = escape_xml(&msg.label),
            );
        }
        MessageKind::Call => {
            let _ = write!(
                out,
                "<line x1=\"{from_x}\" y1=\"{y}\" x2=\"{to_x}\" y2=\"{y}\" stroke=\"#333\" \
                 marker-end=\"url(#arrow)\"/>\
                 <text x=\"{tx}\" y=\"{ty}\" text-anchor=\"middle\">{label}</text>",
                tx = i32::midpoint(from_x, to_x),
                ty = y - 4,
                label = escape_xml(&msg.label),
            );
        }
    }
}

fn draw_frame_box(
    out: &mut String,
    scene: &SequenceScene,
    kind: crate::scene::FrameKind,
    cond: &str,
    top: i32,
    label_y: i32,
    bottom: i32,
) {
    let min_x = scene.participants.iter().map(|l| l.x).min().unwrap_or(PAD) - FRAME_PAD;
    let max_x = scene.participants.iter().map(|l| l.x).max().unwrap_or(PAD) + FRAME_PAD;
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{top}\" width=\"{w}\" height=\"{h}\" fill=\"none\" \
         stroke=\"#77a\" stroke-dasharray=\"2 2\"/>\
         <text x=\"{tx}\" y=\"{label_y}\" fill=\"#558\">{kw} [{cond}]</text>",
        x = min_x,
        w = max_x - min_x,
        h = bottom - top,
        tx = min_x + 6,
        kw = kind.keyword(),
        cond = escape_xml(cond),
    );
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
