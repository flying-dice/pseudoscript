//! SVG layout and rendering.
//!
//! The emit crate owns its layout: [`layout_c4`] and [`layout_sequence`] assign
//! deterministic coordinates to a [`Scene`]'s geometry, and [`render_svg`] turns
//! a laid-out scene into a self-contained SVG document with `std::fmt::Write`
//! string-building. No template engine, headless browser, threads, filesystem,
//! or clock — WASM-clean. The output is correct, deterministic, and readable;
//! not pretty.

use std::fmt::Write as _;

use pseudoscript_layout::sequence::{self, Activation, FragKind, PlacedFragment, PlacedMessage};
use pseudoscript_model::NodeKind;

use crate::c4_render::render_c4;
use crate::scene::{C4Scene, FrameKind, Message, MessageKind, Rect, Scene, SeqItem, SequenceScene};

// C4 layout constants (renderer coordinates). Sequence geometry now lives in the
// `pseudoscript-layout` crate; only the activation-bar width is shared here, to
// draw bars whose width matches the trimmed message endpoints the engine emits.
const PAD: i32 = 20;
const NODE_W: i32 = 160;
const NODE_H: i32 = 60;
const NODE_GAP: i32 = 30;
const BOUNDARY_PAD: i32 = 30;
const ACT_W: i32 = 10; // execution-activation bar width (matches sequence::Metrics::act_w)

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

// --- sequence: scene -> layout IR -------------------------------------------

/// Converts a projected [`SequenceScene`] into the layout crate's input
/// [`Diagram`](sequence::Diagram). Adjacent `alt`/`else` frames (the explicit
/// else and the folded guard-clause fall-through both use this convention) pair
/// into one fragment with a `then` and an `else` section, so the engine splits
/// the box with a divider.
fn to_diagram(scene: &SequenceScene) -> sequence::Diagram {
    sequence::Diagram {
        participants: scene
            .participants
            .iter()
            .map(|l| sequence::Participant {
                id: l.fqn.clone(),
                label: simple_name(&l.fqn).to_owned(),
                kind: kind_token(l.kind).to_owned(),
            })
            .collect(),
        items: to_items(&scene.items),
    }
}

fn to_items(items: &[SeqItem]) -> Vec<sequence::Item> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        match &items[i] {
            SeqItem::Message(msg) => {
                out.push(sequence::Item::Message(to_message(msg)));
                i += 1;
            }
            SeqItem::Frame(frame) => {
                let mut sections = vec![sequence::Section {
                    guard: frame.cond.clone(),
                    body: to_items(&frame.body),
                }];
                // Pair a following `else <cond>` alt-frame in as a second section.
                let mut consumed = 1;
                if frame.kind == FrameKind::Alt
                    && let Some(SeqItem::Frame(next)) = items.get(i + 1)
                    && next.kind == FrameKind::Alt
                    && next.cond.starts_with("else")
                {
                    sections.push(sequence::Section {
                        // The else compartment shows `[else]`; its guard is the
                        // negated `then` condition, redundant on the diagram.
                        guard: String::new(),
                        body: to_items(&next.body),
                    });
                    consumed = 2;
                }
                out.push(sequence::Item::Fragment(sequence::Fragment {
                    kind: to_frag_kind(frame.kind),
                    sections,
                }));
                i += consumed;
            }
        }
    }
    out
}

fn to_message(msg: &Message) -> sequence::Message {
    sequence::Message {
        from: msg.from.clone(),
        to: msg.to.clone(),
        kind: to_msg_kind(msg.kind),
        label: msg.label.clone(),
        detail: msg.detail.clone(),
    }
}

fn to_msg_kind(kind: MessageKind) -> sequence::MsgKind {
    match kind {
        MessageKind::Call => sequence::MsgKind::Call,
        MessageKind::Return => sequence::MsgKind::Return,
        MessageKind::SelfMsg => sequence::MsgKind::SelfMsg,
    }
}

fn to_frag_kind(kind: FrameKind) -> FragKind {
    match kind {
        FrameKind::Alt => FragKind::Alt,
        FrameKind::Loop => FragKind::Loop,
    }
}

/// The lowercase C4-kind token the layout crate carries for head-card styling.
fn kind_token(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Person => "person",
        NodeKind::System => "system",
        NodeKind::Container => "container",
        NodeKind::Component => "component",
        NodeKind::Data => "data",
        NodeKind::Callable => "callable",
    }
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

/// Positions a sequence scene with the layout engine, returning absolute
/// coordinates a renderer (the static SVG here, or the web-ide) draws verbatim.
#[must_use]
pub fn layout_sequence_scene(scene: &SequenceScene) -> sequence::Layout {
    sequence::layout(&to_diagram(scene), &sequence::Metrics::default())
}

fn render_sequence(scene: &SequenceScene) -> String {
    let layout = layout_sequence_scene(scene);
    let mut out = String::new();
    svg_open(&mut out, layout.width, layout.height);

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

    // Combined fragments first (behind the messages they enclose).
    for frag in &layout.fragments {
        draw_frame(&mut out, frag);
    }

    // Execution-activation bars (one per participant, spanning its involvement).
    for act in &layout.activations {
        draw_activation(&mut out, act);
    }

    // Lifeline heads (the kind-coloured C4 card) plus the dashed lifeline. The
    // layout preserves participant order, so the scene supplies each card's kind.
    for (placed, lifeline) in layout.participants.iter().zip(&scene.participants) {
        crate::c4_render::draw_card(
            &mut out,
            placed.card.x,
            placed.card.y,
            placed.card.w,
            placed.card.h,
            lifeline.kind,
            &placed.label,
            None,
        );
        let _ = write!(
            &mut out,
            "<line x1=\"{x}\" y1=\"{top}\" x2=\"{x}\" y2=\"{bot}\" stroke=\"{SEQ_LINE}\" \
             stroke-dasharray=\"2 4\"/>",
            x = placed.lifeline_x,
            top = placed.top,
            bot = placed.bottom,
        );
    }

    // Messages, in reading order.
    for msg in &layout.messages {
        match msg.kind {
            sequence::MsgKind::Call => draw_call(&mut out, msg),
            sequence::MsgKind::SelfMsg => draw_self(&mut out, msg),
            sequence::MsgKind::Return => draw_return(&mut out, msg),
        }
    }

    out.push_str("</svg>");
    out
}

/// A per-participant activation bar. The entry owner's is accented; the rest are
/// plain.
fn draw_activation(out: &mut String, act: &Activation) {
    let (fill, fill_op, stroke, stroke_op) = if act.owner {
        (SEQ_ACCENT, "0.10", SEQ_ACCENT, "0.5")
    } else {
        ("#fff", "1", SEQ_LINE, "1")
    };
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{y}\" width=\"{ACT_W}\" height=\"{h}\" rx=\"2\" fill=\"{fill}\" \
         fill-opacity=\"{fill_op}\" stroke=\"{stroke}\" stroke-opacity=\"{stroke_op}\"/>",
        x = act.x - ACT_W / 2,
        y = act.top,
        h = (act.bottom - act.top).max(ACT_W),
    );
}

/// A call: solid arrow owner → target, numbered, with a short activation bar on
/// the target.
/// A call: solid arrow source → target, numbered at its origin.
fn draw_call(out: &mut String, msg: &PlacedMessage) {
    let _ = write!(
        out,
        "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"{SEQ_INK}\" \
         stroke-width=\"1.4\" marker-end=\"url(#seqcall)\"/>",
        x1 = msg.from_x,
        x2 = msg.to_x,
        y = msg.y,
    );
    step_badge(out, msg.from_x - msg.dir * (ACT_W / 2), msg.y, msg.step);
    seq_label(
        out,
        (i32::midpoint(msg.from_x, msg.to_x), msg.y - 9),
        &msg.label,
        &msg.detail,
        "middle",
        SEQ_INK,
        true,
    );
}

/// A self-message: a rounded loop on the owner's lifeline.
fn draw_self(out: &mut String, msg: &PlacedMessage) {
    let lx = msg.from_x + ACT_W / 2;
    let _ = write!(
        out,
        "<path d=\"M{lx},{y} h34 a6 6 0 0 1 6 6 v8 a6 6 0 0 1 -6 6 h-34\" fill=\"none\" \
         stroke=\"{SEQ_INK}\" stroke-width=\"1.4\" marker-end=\"url(#seqcall)\"/>",
        y = msg.y,
    );
    step_badge(out, msg.from_x, msg.y, msg.step);
    seq_label(
        out,
        (lx + 46, msg.y + 4),
        &msg.label,
        "",
        "start",
        SEQ_INK,
        false,
    );
}

/// A return to the caller: a dashed arrow coloured by its marker (`Ok`/`Some`
/// vs `Err`/`None`), the payload shown as a generic argument of the marker.
fn draw_return(out: &mut String, msg: &PlacedMessage) {
    let (colour, text) = return_style(&msg.label);
    let _ = write!(
        out,
        "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"{colour}\" \
         stroke-width=\"1.3\" stroke-dasharray=\"5 3\" marker-end=\"url(#seqret)\"/>",
        x1 = msg.from_x,
        x2 = msg.to_x,
        y = msg.y,
    );
    let detail = if msg.detail.is_empty() {
        String::new()
    } else if msg.label.is_empty() {
        // A bare value return carries its whole type as a plain suffix.
        format!(" {}", msg.detail)
    } else {
        // A marked return (`Ok`/`Err`/`Some`) carries its payload as a generic
        // argument of the marker: `Ok<Order>`, `Err<DomainError>`.
        format!("<{}>", msg.detail)
    };
    seq_label(
        out,
        (i32::midpoint(msg.from_x, msg.to_x), msg.y - 9),
        &text,
        &detail,
        "middle",
        colour,
        true,
    );
}

/// A combined fragment (`alt`/`loop`): a framed box with a notched operator tab,
/// its guard, and a dashed divider (with the `else` guard) per section split.
fn draw_frame(out: &mut String, frag: &PlacedFragment) {
    let r = frag.rect;
    let op = frag.kind.keyword();
    let tab_w = i32::try_from(op.len()).unwrap_or(3) * 8 + 18;
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{top}\" width=\"{w}\" height=\"{h}\" rx=\"4\" fill=\"{SEQ_INK}\" \
         fill-opacity=\"0.015\" stroke=\"{SEQ_FRAME}\"/>\
         <path d=\"M{x},{top} h{tab_w} l-8,16 h-{tab_inner} z\" fill=\"{SEQ_FRAME}\" \
         fill-opacity=\"0.5\" stroke=\"{SEQ_FRAME}\"/>\
         <text x=\"{ox}\" y=\"{oy}\" font-size=\"11\" font-weight=\"700\" fill=\"{SEQ_INK}\">{op}</text>\
         <text x=\"{gx}\" y=\"{oy}\" font-size=\"11\" fill=\"{SEQ_MUTED}\">[{cond}]</text>",
        x = r.x,
        top = r.y,
        w = r.w,
        h = r.h,
        tab_inner = tab_w - 8,
        ox = r.x + 8,
        oy = r.y + 15,
        gx = r.x + tab_w + 8,
        cond = escape_xml(&frag.label),
    );
    for divider in &frag.dividers {
        let _ = write!(
            out,
            "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"{SEQ_FRAME}\" \
             stroke-dasharray=\"5 3\"/>\
             <text x=\"{tx}\" y=\"{ty}\" font-size=\"11\" font-weight=\"700\" \
             fill=\"{SEQ_MUTED}\">[{guard}]</text>",
            x1 = r.x,
            x2 = r.right(),
            y = divider.y,
            tx = r.x + 8,
            ty = divider.y + 14,
            guard = escape_xml(&divider.guard),
        );
    }
}

/// A small numbered badge left of a message's origin lifeline. `step` is present
/// for calls and self-messages.
fn step_badge(out: &mut String, lifeline_x: i32, y: i32, step: Option<u32>) {
    let Some(step) = step else { return };
    let _ = write!(
        out,
        "<circle cx=\"{cx}\" cy=\"{y}\" r=\"8\" fill=\"{SEQ_ACCENT}\"/>\
         <text x=\"{cx}\" y=\"{ty}\" text-anchor=\"middle\" font-size=\"10\" font-weight=\"700\" \
         fill=\"#fff\">{step}</text>",
        cx = lifeline_x - ACT_W / 2 - 12,
        ty = y + 3,
    );
}

/// A message label with a faint backing pill for legibility over lifelines and
/// activation bars. `detail` (a call signature or a return's concrete type) is
/// appended in a dimmed tspan after the name; empty when there is none.
fn seq_label(
    out: &mut String,
    (x, y): (i32, i32),
    text: &str,
    detail: &str,
    anchor: &str,
    colour: &str,
    pill: bool,
) {
    let label = escape_xml(text);
    let chars = text.chars().count() + detail.chars().count();
    if pill && chars > 0 {
        let w = i32::try_from(chars).unwrap_or(0) * 7 + 12;
        let _ = write!(
            out,
            "<rect x=\"{rx}\" y=\"{ry}\" width=\"{w}\" height=\"17\" rx=\"4\" fill=\"#fff\" \
             fill-opacity=\"0.92\"/>",
            rx = x - w / 2,
            ry = y - 12,
        );
    }
    let detail_span = if detail.is_empty() {
        String::new()
    } else {
        format!("<tspan fill=\"{SEQ_MUTED}\">{}</tspan>", escape_xml(detail))
    };
    let _ = write!(
        out,
        "<text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" font-size=\"12.5\" \
         fill=\"{colour}\">{label}{detail_span}</text>",
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
