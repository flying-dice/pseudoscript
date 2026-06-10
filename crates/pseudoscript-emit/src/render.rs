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
use crate::scene::{
    C4Scene, DataScene, FeatureScene, FrameKind, Message, MessageKind, Rect, Scene, SeqItem,
    SequenceScene,
};

// C4 layout constants (renderer coordinates). Sequence geometry now lives in the
// `pseudoscript-layout` crate; only the activation-bar width is shared here, to
// draw bars whose width matches the trimmed message endpoints the engine emits.
const PAD: i32 = 20;
const NODE_W: i32 = 160;
const NODE_H: i32 = 60;
const NODE_GAP: i32 = 30;
const BOUNDARY_PAD: i32 = 30;
const ACT_W: i32 = 10; // execution-activation bar width (matches sequence::Metrics::act_w)
// Lifeline-card text inset: left rule + text pad, matching `draw_card`'s `tx` and
// the layout's `head::TEXT_INSET` so the parent path / summary align with the
// name above them.
const CARD_TEXT_X: i32 = 19;

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
                summary: l.summary.clone(),
                parent_path: l.parent_path.clone(),
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

/// The colour theme a diagram renders in. `Light` reproduces the original
/// ink-on-paper palette byte-for-byte; `Dark` swaps the structural colours
/// (ink, hairlines, card/boundary fills, plates) for a dark surface while
/// keeping the per-kind accent colours, mirroring the doc site's two modes.
/// `Adaptive` paints every palette role as a `var(--pds-<role>, <light value>)`
/// CSS custom property, deferring the colour choice to the host stylesheet —
/// the doc site's theme-following diagrams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    Adaptive,
}

/// Every colour the SVG emitters draw with, by role. Two instances exist
/// ([`LIGHT`]/[`DARK`]); the active one is held in a thread-local for the
/// duration of one render so the many emit helpers need no extra argument.
pub(crate) struct Palette {
    /// Primary ink: text, lines, the C4 arrowhead.
    pub ink: &'static str,
    /// Sequence-arrowhead ink (a slightly softer near-black in light mode).
    pub arrow_seq: &'static str,
    /// Hairlines: card borders and dashed lifelines.
    pub hairline: &'static str,
    /// Secondary text: descriptions, guards, return markers.
    pub muted: &'static str,
    /// Combined-fragment frame strokes/tabs.
    pub frame: &'static str,
    /// The entry/owner accent (activation, step badges).
    pub accent: &'static str,
    /// Success-return colour (`Ok`/`Some`).
    pub ok: &'static str,
    /// Error-return colour (`Err`/`None`).
    pub err: &'static str,
    /// A card's interior fill.
    pub card_fill: &'static str,
    /// A boundary frame's fill.
    pub boundary_fill: &'static str,
    /// The card drop-shadow colour.
    pub shadow: &'static str,
    /// A plain (non-owner) activation bar's fill.
    pub act_fill: &'static str,
    /// Text drawn on an accent fill (step numbers) — stays light in both themes.
    pub on_accent: &'static str,
    /// The faint backing pill behind a sequence label.
    pub pill: &'static str,
    /// The plate behind a C4 edge label (carries its own alpha).
    pub edge_plate: &'static str,
}

/// The original ink-on-paper palette — light output is unchanged from before
/// the theme split, so golden SVGs still match.
pub(crate) static LIGHT: Palette = Palette {
    ink: "#2a2f3a",
    arrow_seq: "#333",
    hairline: "#c3c8d2",
    muted: "#6b7280",
    frame: "#aab0bd",
    accent: "#e8431f",
    ok: "#0f9d8a",
    err: "#d6432a",
    card_fill: "#ffffff",
    boundary_fill: "#f6f7fa",
    shadow: "#1a1f2a",
    act_fill: "#fff",
    on_accent: "#fff",
    pill: "#fff",
    edge_plate: "#ffffffe6",
};

/// The dark surface palette: light ink, dark cards/plates, muted hairlines; the
/// per-kind accents (set elsewhere) carry through unchanged.
pub(crate) static DARK: Palette = Palette {
    ink: "#d4d7dd",
    arrow_seq: "#b7bbc3",
    hairline: "#44474e",
    muted: "#9a9ea8",
    frame: "#565a62",
    accent: "#ff5c38",
    ok: "#23c2ab",
    err: "#f0563b",
    card_fill: "#2b2d31",
    boundary_fill: "#26282d",
    shadow: "#000000",
    act_fill: "#3b3e45",
    on_accent: "#fff",
    pill: "#2b2d31",
    edge_plate: "#2b2d31e6",
};

/// The adaptive palette: every role is a `--pds-*` CSS custom property with the
/// light value as fallback, so an adaptive SVG renders light standalone and
/// follows whatever palette the host stylesheet (or an embedded
/// [`adaptive_style_block`]) assigns. The `tests` module pins each entry to
/// `var(--pds-<role>, <LIGHT value>)` so the fallbacks cannot drift.
pub(crate) static ADAPTIVE: Palette = Palette {
    ink: "var(--pds-ink, #2a2f3a)",
    arrow_seq: "var(--pds-arrow-seq, #333)",
    hairline: "var(--pds-hairline, #c3c8d2)",
    muted: "var(--pds-muted, #6b7280)",
    frame: "var(--pds-frame, #aab0bd)",
    accent: "var(--pds-accent, #e8431f)",
    ok: "var(--pds-ok, #0f9d8a)",
    err: "var(--pds-err, #d6432a)",
    card_fill: "var(--pds-card-fill, #ffffff)",
    boundary_fill: "var(--pds-boundary-fill, #f6f7fa)",
    shadow: "var(--pds-shadow, #1a1f2a)",
    act_fill: "var(--pds-act-fill, #fff)",
    on_accent: "var(--pds-on-accent, #fff)",
    pill: "var(--pds-pill, #fff)",
    edge_plate: "var(--pds-edge-plate, #ffffffe6)",
};

thread_local! {
    /// The palette in effect for the current render (defaults to light).
    static PALETTE: std::cell::Cell<&'static Palette> = const { std::cell::Cell::new(&LIGHT) };
}

/// The palette the emit helpers should draw with right now.
pub(crate) fn pal() -> &'static Palette {
    PALETTE.with(std::cell::Cell::get)
}

/// Renders a laid-out [`Scene`] to a self-contained SVG document (light theme).
#[must_use]
pub fn render_svg(scene: &Scene) -> String {
    render_svg_themed(scene, Theme::Light)
}

/// Renders a laid-out [`Scene`] to a self-contained SVG document in `theme`.
/// Sets the thread-local palette for the duration of the render and restores
/// light afterwards, so a default `render_svg` elsewhere is unaffected.
#[must_use]
pub fn render_svg_themed(scene: &Scene, theme: Theme) -> String {
    let palette: &'static Palette = match theme {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
        Theme::Adaptive => &ADAPTIVE,
    };
    PALETTE.with(|p| p.set(palette));
    let svg = match scene {
        Scene::C4(c4) => render_c4(c4),
        Scene::Sequence(seq) => render_sequence(seq),
        Scene::Data(data) => render_data(data),
        Scene::Feature(feature) => render_feature(feature),
    };
    PALETTE.with(|p| p.set(&LIGHT));
    svg
}

/// The style block a standalone adaptive SVG embeds so the file follows the OS
/// theme outside any host stylesheet: a `prefers-color-scheme: dark` media
/// query assigning the dark palette to the `--pds-*` variables. Built from
/// [`DARK`] so the two cannot drift.
#[must_use]
pub fn adaptive_style_block() -> String {
    let d = &DARK;
    format!(
        "<style>@media (prefers-color-scheme: dark){{:root{{\
         --pds-ink:{};--pds-arrow-seq:{};--pds-hairline:{};--pds-muted:{};\
         --pds-frame:{};--pds-accent:{};--pds-ok:{};--pds-err:{};\
         --pds-card-fill:{};--pds-boundary-fill:{};--pds-shadow:{};\
         --pds-act-fill:{};--pds-on-accent:{};--pds-pill:{};--pds-edge-plate:{}\
         }}}}</style>",
        d.ink,
        d.arrow_seq,
        d.hairline,
        d.muted,
        d.frame,
        d.accent,
        d.ok,
        d.err,
        d.card_fill,
        d.boundary_fill,
        d.shadow,
        d.act_fill,
        d.on_accent,
        d.pill,
        d.edge_plate,
    )
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
    let _ = write!(
        out,
        "<defs><marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"3\" \
         orient=\"auto\" markerUnits=\"strokeWidth\"><path d=\"M0,0 L9,3 L0,6 z\" \
         fill=\"{}\"/></marker></defs>",
        pal().arrow_seq,
    );
    // Set the font on a group: `<text>` inherits it here even in renderers (e.g.
    // JSVG, used by the JetBrains plugin) that don't inherit it from the root
    // `<svg>`. Concrete families first so font-less rasterisers still resolve one.
    out.push_str(SVG_FONT_GROUP);
}

/// Opens the font-bearing group; pair with [`svg_close`]. Shared with the C4
/// renderer so both diagram kinds carry the font on a group, not the root.
pub(crate) const SVG_FONT_GROUP: &str = "<g font-family=\"Helvetica, Arial, sans-serif\">";

/// Closes the font group and the document opened by `svg_open`.
pub(crate) fn svg_close(out: &mut String) {
    out.push_str("</g></svg>");
}

/// Positions a sequence scene with the layout engine, returning absolute
/// coordinates a renderer (the static SVG here, or the web-ide) draws verbatim.
#[must_use]
pub fn layout_sequence_scene(scene: &SequenceScene) -> sequence::Layout {
    sequence::layout(&to_diagram(scene), &sequence::Metrics::default())
}

fn render_sequence(scene: &SequenceScene) -> String {
    #[allow(non_snake_case)]
    let (SEQ_INK, SEQ_MUTED, SEQ_LINE) = (pal().ink, pal().muted, pal().hairline);
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
        // Dimmed parent path (container/component) then the wrapped summary,
        // under the name. Baselines come from `sequence::head` so they line up
        // with the card height the engine computed.
        let tx = placed.card.x + CARD_TEXT_X;
        if let Some(parent) = &placed.parent_path {
            let _ = write!(
                &mut out,
                "<text x=\"{tx}\" y=\"{y}\" font-size=\"11\" fill=\"{SEQ_MUTED}\">{parent}</text>",
                y = placed.card.y + sequence::head::PARENT_Y,
                parent = escape_xml(parent),
            );
        }
        let desc_top = sequence::head::DESC_TOP_Y
            + if placed.parent_path.is_some() {
                sequence::head::DESC_SHIFT_Y
            } else {
                0
            };
        for (i, line) in placed.summary_lines.iter().enumerate() {
            let _ = write!(
                &mut out,
                "<text x=\"{tx}\" y=\"{y}\" font-size=\"11.5\" fill=\"{SEQ_MUTED}\">{line}</text>",
                y = placed.card.y
                    + desc_top
                    + i32::try_from(i).unwrap_or(0) * sequence::head::DESC_LINE_H,
                line = escape_xml(line),
            );
        }
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

    svg_close(&mut out);
    out
}

/// A per-participant activation bar. The entry owner's is accented; the rest are
/// plain.
fn draw_activation(out: &mut String, act: &Activation) {
    let (fill, fill_op, stroke, stroke_op) = if act.owner {
        (pal().accent, "0.10", pal().accent, "0.5")
    } else {
        (pal().act_fill, "1", pal().hairline, "1")
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
    #[allow(non_snake_case)]
    let SEQ_INK = pal().ink;
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
    #[allow(non_snake_case)]
    let SEQ_INK = pal().ink;
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
    #[allow(non_snake_case)]
    let (SEQ_INK, SEQ_FRAME, SEQ_MUTED) = (pal().ink, pal().frame, pal().muted);
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
    #[allow(non_snake_case)]
    let SEQ_ACCENT = pal().accent;
    let on_accent = pal().on_accent;
    let _ = write!(
        out,
        "<circle cx=\"{cx}\" cy=\"{y}\" r=\"8\" fill=\"{SEQ_ACCENT}\"/>\
         <text x=\"{cx}\" y=\"{ty}\" text-anchor=\"middle\" font-size=\"10\" font-weight=\"700\" \
         fill=\"{on_accent}\">{step}</text>",
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
    #[allow(non_snake_case)]
    let SEQ_MUTED = pal().muted;
    let label = escape_xml(text);
    let chars = text.chars().count() + detail.chars().count();
    if pill && chars > 0 {
        let w = i32::try_from(chars).unwrap_or(0) * 7 + 8;
        let _ = write!(
            out,
            "<rect x=\"{rx}\" y=\"{ry}\" width=\"{w}\" height=\"14\" rx=\"4\" fill=\"{fill}\" \
             fill-opacity=\"0.92\"/>",
            rx = x - w / 2,
            ry = y - 11,
            fill = pal().pill,
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
        "Ok" | "Some" => (pal().ok, format!("\u{21a9} {marker}")),
        "Err" | "None" => (pal().err, format!("\u{21a9} {marker}")),
        "" => (pal().muted, "\u{21a9} return".to_owned()),
        other => (pal().muted, format!("\u{21a9} {other}")),
    }
}

// --- data (ER) layout + rendering -------------------------------------------

const ENTITY_HDR: i32 = 40; // header band: eyebrow + name
const ENTITY_ROW_H: i32 = 22; // per-field row height
const ENTITY_PAD_B: i32 = 12; // card bottom padding
const ENTITY_MIN_W: i32 = 150;
const ENTITY_MAX_W: i32 = 440;
const ENTITY_CHAR_W: i32 = 8; // approx px per row character (12.5px mono advance)
const ENTITY_NAME_CHAR_W: i32 = 10; // approx px per header-name char (15px bold display)
const ENTITY_COL_GAP: i32 = 96; // gap between the focal card and the referenced column
const ENTITY_VGAP: i32 = 24; // vertical gap between referenced cards
/// The data-entity accent (matches the C4 `data` card colour).
const DATA_ACCENT: &str = "#9333ea";

/// The card width for an entity: wide enough for the widest row (mono) and the
/// header name (the bold display font, wider per character), clamped.
fn entity_width(entity: &crate::scene::DataEntity) -> i32 {
    let row_chars = |r: &crate::scene::EntityRow| {
        if r.ty.is_empty() {
            r.name.chars().count()
        } else {
            // The row renders `name : ty` — the ` : ` separator is three chars.
            r.name.chars().count() + 3 + r.ty.chars().count()
        }
    };
    let chars = |n: usize| i32::try_from(n).unwrap_or(0);
    let rows_w = entity.rows.iter().map(row_chars).max().unwrap_or(0);
    // The name is drawn in the 15px bold display font; size it at its own advance
    // so a long type name is not clipped by the card edge.
    let name_w = chars(entity.label.chars().count()) * ENTITY_NAME_CHAR_W;
    (chars(rows_w) * ENTITY_CHAR_W)
        .max(name_w)
        .saturating_add(40)
        .clamp(ENTITY_MIN_W, ENTITY_MAX_W)
}

/// The card height for an entity: the header band plus a row per field.
fn entity_height(entity: &crate::scene::DataEntity) -> i32 {
    if entity.rows.is_empty() {
        ENTITY_HDR + ENTITY_PAD_B
    } else {
        ENTITY_HDR + i32::try_from(entity.rows.len()).unwrap_or(0) * ENTITY_ROW_H + ENTITY_PAD_B
    }
}

/// Positions a [`DataScene`]: the focal entity top-left, its referenced types in
/// a column to the right. Deterministic; the same geometry [`render_data`] draws.
#[must_use]
pub fn layout_data_scene(scene: &DataScene) -> DataScene {
    let mut scene = scene.clone();
    if let Some(focal) = scene.entities.first_mut() {
        focal.rect = Rect {
            x: PAD,
            y: PAD,
            w: entity_width(focal),
            h: entity_height(focal),
        };
    }
    let (fx, fw) = scene
        .entities
        .first()
        .map_or((PAD, ENTITY_MIN_W), |e| (e.rect.x, e.rect.w));
    let col_x = fx + fw + ENTITY_COL_GAP;
    let mut y = PAD;
    for entity in scene.entities.iter_mut().skip(1) {
        let h = entity_height(entity);
        entity.rect = Rect {
            x: col_x,
            y,
            w: entity_width(entity),
            h,
        };
        y += h + ENTITY_VGAP;
    }
    let extent = |f: fn(&Rect) -> i32| {
        scene
            .entities
            .iter()
            .map(|e| f(&e.rect))
            .max()
            .unwrap_or(PAD)
    };
    scene.width = extent(|r| r.x + r.w) + PAD;
    scene.height = extent(|r| r.y + r.h) + PAD;
    scene
}

fn render_data(scene: &DataScene) -> String {
    let scene = layout_data_scene(scene);
    let mut out = String::new();
    svg_open(&mut out, scene.width.max(PAD), scene.height.max(PAD));
    // Reference links first, behind the cards they connect.
    for link in &scene.links {
        draw_data_link(&mut out, &scene, link);
    }
    for entity in &scene.entities {
        draw_entity(&mut out, entity);
    }
    svg_close(&mut out);
    out
}

/// Draws an entity card: a coloured left rule, an UPPERCASE form eyebrow, the
/// bold type name, and one row per field (`name : ty`), a dot marking a row that
/// references another type.
fn draw_entity(out: &mut String, entity: &crate::scene::DataEntity) {
    #[allow(non_snake_case)]
    let (CARD_FILL, BORDER, INK, MUTED) = (pal().card_fill, pal().hairline, pal().ink, pal().muted);
    let r = entity.rect;
    // The accent shows as a clean left rule, the same way a C4 card draws it: a
    // base accent rounded rect, then a card-fill interior with the left edge
    // square and right corners rounded, so only a BAR_WIDTH strip of the base
    // shows. A neutral border (accent when focal) sits over the whole card.
    let rad = 8;
    let (ix, right, bottom) = (r.x + 5, r.x + r.w, r.y + r.h);
    let border = if entity.focal { DATA_ACCENT } else { BORDER };
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{rad}\" fill=\"{DATA_ACCENT}\"/>\
         <path d=\"M{ix},{y} H{rl} A{rad},{rad} 0 0 1 {right},{ry} V{by} A{rad},{rad} 0 0 1 {rl},{bottom} H{ix} Z\" fill=\"{CARD_FILL}\"/>\
         <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{rad}\" fill=\"none\" stroke=\"{border}\"/>",
        x = r.x,
        y = r.y,
        w = r.w,
        h = r.h,
        rl = right - rad,
        ry = r.y + rad,
        by = bottom - rad,
    );
    let tx = r.x + 16;
    let _ = write!(
        out,
        "<text x=\"{tx}\" y=\"{ey}\" font-size=\"10\" letter-spacing=\"1.5\" font-weight=\"600\" \
         fill=\"{DATA_ACCENT}\">{eyebrow}</text>\
         <text x=\"{tx}\" y=\"{ny}\" font-size=\"15\" font-weight=\"700\" fill=\"{INK}\">{name}</text>",
        ey = r.y + 18,
        ny = r.y + 33,
        eyebrow = escape_xml(&entity.form.keyword().to_uppercase()),
        name = escape_xml(&entity.label),
    );
    for (i, row) in entity.rows.iter().enumerate() {
        let ry = r.y + ENTITY_HDR + i32::try_from(i).unwrap_or(0) * ENTITY_ROW_H + 15;
        // Highlight the row: field name in ink, the `:` dimmed, the type in the
        // data accent (matching the editor's type colour and the reference arrow).
        // A union variant row (no type) is itself a type token.
        if row.ty.is_empty() {
            let _ = write!(
                out,
                "<text x=\"{tx}\" y=\"{ry}\" font-size=\"12.5\" fill=\"{DATA_ACCENT}\">{name}</text>",
                name = escape_xml(&row.name),
            );
        } else {
            let _ = write!(
                out,
                // Non-breaking space after the colon so SVG whitespace collapsing
                // keeps the `name: type` gap at the tspan boundary.
                "<text x=\"{tx}\" y=\"{ry}\" font-size=\"12.5\">\
                 <tspan fill=\"{INK}\">{name}</tspan>\
                 <tspan fill=\"{MUTED}\">:\u{00a0}</tspan>\
                 <tspan fill=\"{DATA_ACCENT}\">{ty}</tspan></text>",
                name = escape_xml(&row.name),
                ty = escape_xml(&row.ty),
            );
        }
        if row.target.is_some() {
            let _ = write!(
                out,
                "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"3.5\" fill=\"{DATA_ACCENT}\"/>",
                cx = r.x + r.w - 9,
                cy = ry - 4,
            );
        }
    }
}

/// Draws a reference link: an elbow from the referencing field's row to the
/// left edge of the referenced card, arrowhead at the target.
fn draw_data_link(out: &mut String, scene: &DataScene, link: &crate::scene::DataLink) {
    let Some(from) = scene.entities.iter().find(|e| e.fqn == link.from) else {
        return;
    };
    let Some(to) = scene.entities.iter().find(|e| e.fqn == link.to) else {
        return;
    };
    let row_idx = from
        .rows
        .iter()
        .position(|r| r.name == link.field)
        .unwrap_or(0);
    let y1 = from.rect.y + ENTITY_HDR + i32::try_from(row_idx).unwrap_or(0) * ENTITY_ROW_H + 11;
    let x1 = from.rect.x + from.rect.w;
    let x2 = to.rect.x;
    let y2 = to.rect.y + to.rect.h / 2;
    let midx = i32::midpoint(x1, x2);
    #[allow(non_snake_case)]
    let STROKE = pal().arrow_seq;
    let _ = write!(
        out,
        "<path d=\"M{x1},{y1} H{midx} V{y2} H{x2}\" fill=\"none\" stroke=\"{STROKE}\" \
         marker-end=\"url(#arrow)\"/>",
    );
}

// --- feature (flow) layout + rendering --------------------------------------

const STEP_W: i32 = 340; // fixed step-box width; text wraps within it
const STEP_PAD_X: i32 = 16; // text inset from each side
const STEP_TEXT_TOP: i32 = 36; // first prose-line baseline within the box
const STEP_LINE_H: i32 = 17; // wrapped-line advance
const STEP_PAD_B: i32 = 12; // padding below the last line
const STEP_CHAR_W: i32 = 8; // approx px per prose char (13px mono advance ≈ 7.8)
const STEP_GAP: i32 = 28;
const FEATURE_HDR: i32 = 66; // header band: eyebrow + name + target

/// The per-line character budget a step box wraps its prose to.
fn step_max_chars() -> usize {
    usize::try_from((STEP_W - 2 * STEP_PAD_X) / STEP_CHAR_W)
        .unwrap_or(1)
        .max(1)
}

/// The box height for a step with `lines` wrapped prose lines.
fn step_height(lines: i32) -> i32 {
    STEP_TEXT_TOP + (lines - 1).max(0) * STEP_LINE_H + STEP_PAD_B
}

/// Greedy word-wrap of `text` to at most `max_chars` per line. An over-long word
/// keeps its own line rather than being split.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let max = max_chars.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.chars().count() + 1 + word.chars().count() <= max {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Positions a [`FeatureScene`]: the steps stacked top-to-bottom under a header,
/// each box's height grown to fit its wrapped prose.
#[must_use]
pub fn layout_feature_scene(scene: &FeatureScene) -> FeatureScene {
    let mut scene = scene.clone();
    let max_chars = step_max_chars();
    let mut y = FEATURE_HDR;
    for step in &mut scene.steps {
        let lines = i32::try_from(wrap_text(&step.text, max_chars).len()).unwrap_or(1);
        let h = step_height(lines);
        step.rect = Rect {
            x: PAD,
            y,
            w: STEP_W,
            h,
        };
        y += h + STEP_GAP;
    }
    // Fit the header name (17px bold display) too, so a long feature name is not
    // clipped at the canvas edge.
    let name_w = i32::try_from(scene.name.chars().count()).unwrap_or(0) * 11 + 2 * PAD;
    scene.width = (STEP_W + 2 * PAD).max(name_w);
    scene.height = (y - STEP_GAP + PAD).max(FEATURE_HDR + PAD);
    scene
}

/// The accent colour for a step keyword.
fn step_color(keyword: &str) -> &'static str {
    match keyword {
        "given" => "#4f72f0",
        "when" => "#c77f10",
        "then" => "#0f9d8a",
        _ => pal().muted,
    }
}

fn render_feature(scene: &FeatureScene) -> String {
    let scene = layout_feature_scene(scene);
    #[allow(non_snake_case)]
    let (INK, MUTED) = (pal().ink, pal().muted);
    let mut out = String::new();
    svg_open(&mut out, scene.width.max(PAD), scene.height.max(PAD));
    let _ = write!(
        &mut out,
        "<text x=\"{PAD}\" y=\"22\" font-size=\"10\" letter-spacing=\"2\" fill=\"{MUTED}\">FEATURE</text>\
         <text x=\"{PAD}\" y=\"44\" font-size=\"17\" font-weight=\"700\" fill=\"{INK}\">{name}</text>\
         <text x=\"{PAD}\" y=\"60\" font-size=\"11.5\" fill=\"{MUTED}\">for {target}</text>",
        name = escape_xml(&scene.name),
        target = escape_xml(&scene.target_label),
    );
    for pair in scene.steps.windows(2) {
        let (a, b) = (pair[0].rect, pair[1].rect);
        let _ = write!(
            &mut out,
            "<line x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"{MUTED}\" \
             marker-end=\"url(#arrow)\"/>",
            x = a.x + a.w / 2,
            y1 = a.y + a.h,
            y2 = b.y,
        );
    }
    for step in &scene.steps {
        draw_step(&mut out, step);
    }
    svg_close(&mut out);
    out
}

/// Draws a step box: a coloured left rule, an UPPERCASE keyword eyebrow, and the
/// step prose.
fn draw_step(out: &mut String, step: &crate::scene::FeatureStepNode) {
    #[allow(non_snake_case)]
    let (CARD_FILL, INK) = (pal().card_fill, pal().ink);
    let accent = step_color(&step.keyword);
    let r = step.rect;
    let tx = r.x + STEP_PAD_X;
    let _ = write!(
        out,
        "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"8\" fill=\"{CARD_FILL}\" \
         stroke=\"{hair}\"/>\
         <rect x=\"{x}\" y=\"{y}\" width=\"5\" height=\"{h}\" rx=\"2\" fill=\"{accent}\"/>\
         <text x=\"{tx}\" y=\"{ky}\" font-size=\"10\" letter-spacing=\"1.5\" font-weight=\"600\" \
         fill=\"{accent}\">{kw}</text>",
        x = r.x,
        y = r.y,
        w = r.w,
        h = r.h,
        hair = pal().hairline,
        ky = r.y + 18,
        kw = escape_xml(&step.keyword.to_uppercase()),
    );
    // The prose, wrapped to the box width, one `<tspan>` per line.
    let _ = write!(
        out,
        "<text x=\"{tx}\" y=\"{ty}\" font-size=\"13\" fill=\"{INK}\">",
        ty = r.y + STEP_TEXT_TOP,
    );
    for (i, line) in wrap_text(&step.text, step_max_chars()).iter().enumerate() {
        let dy = if i == 0 { 0 } else { STEP_LINE_H };
        let _ = write!(
            out,
            "<tspan x=\"{tx}\" dy=\"{dy}\">{}</tspan>",
            escape_xml(line)
        );
    }
    out.push_str("</text>");
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
    fn adaptive_palette_wraps_light_fallbacks() {
        // Every adaptive role must be `var(--pds-<role>, <LIGHT value>)` so a
        // standalone adaptive SVG renders identically to the light theme.
        let roles = [
            ("ink", ADAPTIVE.ink, LIGHT.ink),
            ("arrow-seq", ADAPTIVE.arrow_seq, LIGHT.arrow_seq),
            ("hairline", ADAPTIVE.hairline, LIGHT.hairline),
            ("muted", ADAPTIVE.muted, LIGHT.muted),
            ("frame", ADAPTIVE.frame, LIGHT.frame),
            ("accent", ADAPTIVE.accent, LIGHT.accent),
            ("ok", ADAPTIVE.ok, LIGHT.ok),
            ("err", ADAPTIVE.err, LIGHT.err),
            ("card-fill", ADAPTIVE.card_fill, LIGHT.card_fill),
            ("boundary-fill", ADAPTIVE.boundary_fill, LIGHT.boundary_fill),
            ("shadow", ADAPTIVE.shadow, LIGHT.shadow),
            ("act-fill", ADAPTIVE.act_fill, LIGHT.act_fill),
            ("on-accent", ADAPTIVE.on_accent, LIGHT.on_accent),
            ("pill", ADAPTIVE.pill, LIGHT.pill),
            ("edge-plate", ADAPTIVE.edge_plate, LIGHT.edge_plate),
        ];
        for (role, adaptive, light) in roles {
            assert_eq!(adaptive, format!("var(--pds-{role}, {light})"));
        }
    }

    #[test]
    fn adaptive_render_paints_css_variables_and_restores_light() {
        use crate::scene::{Lifeline, SequenceScene};
        let scene = Scene::Sequence(SequenceScene {
            entry: "m::Comp::run".to_owned(),
            participants: vec![Lifeline {
                fqn: "m::Comp".to_owned(),
                kind: NodeKind::Component,
                summary: None,
                parent_path: None,
            }],
            items: Vec::new(),
        });
        let adaptive = render_svg_themed(&scene, Theme::Adaptive);
        assert!(
            adaptive.contains("var(--pds-ink"),
            "roles paint as variables"
        );
        // The thread-local restores: a plain render afterwards is light again.
        assert_eq!(render_svg(&scene), render_svg_themed(&scene, Theme::Light));
        assert!(!render_svg(&scene).contains("var(--pds-"));
    }

    #[test]
    fn adaptive_style_block_assigns_the_dark_palette() {
        let block = adaptive_style_block();
        assert!(block.starts_with("<style>@media (prefers-color-scheme: dark)"));
        assert!(block.contains(&format!("--pds-ink:{}", DARK.ink)));
        assert!(block.contains(&format!("--pds-edge-plate:{}", DARK.edge_plate)));
    }

    #[test]
    fn simple_name_takes_leaf() {
        assert_eq!(simple_name("a::b::C"), "C");
        assert_eq!(simple_name("event:a::B"), "event:a::B");
    }

    #[test]
    fn sequence_head_card_draws_parent_path_and_summary() {
        use crate::scene::{Lifeline, SequenceScene};
        let scene = SequenceScene {
            entry: "m::Comp::run".to_owned(),
            participants: vec![Lifeline {
                fqn: "m::Comp".to_owned(),
                kind: NodeKind::Component,
                summary: Some("Validates the order before queueing.".to_owned()),
                parent_path: Some("Shop::Api".to_owned()),
            }],
            items: Vec::new(),
        };
        let svg = render_sequence(&scene);
        assert!(svg.contains("Shop::Api"), "parent path drawn");
        assert!(svg.contains("Validates the order"), "summary drawn");
    }
}
