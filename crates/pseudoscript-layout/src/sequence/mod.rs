//! The sequence-diagram layout engine.
//!
//! Implements the standard sequence-diagram layout pattern (as used by the
//! mainstream open-source tools) in three passes:
//!
//! 1. **Measure** — participant card widths from their titles, and per-message
//!    label widths from the rendered text.
//! 2. **Space columns** — size each inter-lifeline gap independently: a gap is
//!    widened only by the labels that actually span it (a single wide label
//!    widens its own lane, not every lane), floored at card + gutter so no card
//!    overlaps its neighbour, then place lifelines.
//! 3. **Walk vertically** — advance a `y` cursor down the items, emitting
//!    messages, growing fragment boxes around their sections (with a divider
//!    between sections), and recording each participant's activation span.
//!
//! The algorithm is original Rust; only the well-known *approach* is shared with
//! other tools.

pub mod diagram;
pub mod layout;

use std::collections::HashMap;

pub use diagram::{Diagram, FragKind, Fragment, Item, Message, MsgKind, Participant, Section};
pub use layout::{Activation, Divider, Layout, PlacedFragment, PlacedMessage, PlacedParticipant};

use crate::Projection;
use crate::geom::{Bounds, Rect};
use crate::text::TextMetrics;

/// Spacing constants for the sequence engine, in renderer coordinates. The
/// [`Default`] matches the web-ide's mono font so positions render identically
/// in the browser and the static SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub text: TextMetrics,
    /// Canvas margin on every side.
    pub pad: i32,
    /// Minimum participant card width.
    pub node_w_min: i32,
    /// Y where the head cards sit.
    pub lifeline_top: i32,
    /// Vertical advance per call/return row.
    pub msg_gap: i32,
    /// Extra height a self-message loop adds.
    pub self_extra: i32,
    /// Operator-tab band: clearance above a fragment's first row.
    pub frame_head: i32,
    /// Padding below a fragment's last row.
    pub frame_foot: i32,
    /// Gap after a fragment, so the next message clears its border.
    pub frame_gap: i32,
    /// Gap below a section before its divider.
    pub else_pad: i32,
    /// Band below a divider, clearing the else guard.
    pub else_head: i32,
    /// Execution-activation bar width.
    pub act_w: i32,
    /// Return-stub length when a target lifeline is unresolved.
    pub ret_stub: i32,
    /// Horizontal padding inside a fragment border.
    pub frame_pad: i32,
    /// Where a self-message label starts, right of the lifeline.
    pub self_offset: i32,
    /// Minimum gutter between adjacent cards.
    pub col_gutter: i32,
    /// Horizontal padding added to a card around its title.
    pub card_pad: i32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            text: TextMetrics::default(),
            pad: 20,
            node_w_min: 168,
            lifeline_top: 64,
            msg_gap: 44,
            self_extra: 20,
            frame_head: 38,
            frame_foot: 14,
            frame_gap: 16,
            else_pad: 12,
            else_head: 26,
            act_w: 10,
            ret_stub: 46,
            frame_pad: 18,
            self_offset: 46,
            col_gutter: 48,
            card_pad: 28,
        }
    }
}

impl Metrics {
    /// The pill width a message's rendered text occupies — mirrors the
    /// consumer's label formatting (`method` + signature; `↩ marker` + `<type>`)
    /// so the lane is sized to what actually gets drawn.
    #[must_use]
    pub fn message_pill_width(&self, msg: &Message) -> i32 {
        match msg.kind {
            MsgKind::Call => self
                .text
                .label_width(&format!("{}{}", msg.label, msg.detail)),
            MsgKind::SelfMsg => self.text.label_width(&msg.label),
            MsgKind::Return => {
                let marker = if msg.label.is_empty() {
                    "\u{21a9} return".to_owned()
                } else {
                    format!("\u{21a9} {}", msg.label)
                };
                let ty = if msg.detail.is_empty() {
                    String::new()
                } else if msg.label.is_empty() {
                    format!(" {}", msg.detail)
                } else {
                    format!("<{}>", msg.detail)
                };
                self.text.label_width(&format!("{marker}{ty}"))
            }
        }
    }
}

/// Lifeline head-card text geometry, in pixels from the card's top edge. Shared
/// with the SVG renderer (`pseudoscript-emit`) so the card height computed here
/// and the text baselines drawn there stay in lockstep across crates. The
/// renderer draws the eyebrow, name, optional dimmed parent path, then the
/// wrapped summary, each at the baseline these constants pin.
pub mod head {
    /// Eyebrow (kind) baseline.
    pub const EYEBROW_Y: i32 = 20;
    /// Name baseline.
    pub const TITLE_Y: i32 = 40;
    /// Dimmed parent-path baseline, when present.
    pub const PARENT_Y: i32 = 56;
    /// First summary-line baseline when no parent path is shown.
    pub const DESC_TOP_Y: i32 = 58;
    /// Extra downward shift applied to the summary lines when a parent path sits
    /// above them.
    pub const DESC_SHIFT_Y: i32 = 16;
    /// Vertical advance per summary line.
    pub const DESC_LINE_H: i32 = 15;
    /// Padding below the last drawn baseline.
    pub const BOTTOM_PAD: i32 = 14;
    /// Minimum card height (an eyebrow + name with nothing extra).
    pub const MIN_H: i32 = 60;
    /// Summary wraps to at most this many lines.
    pub const MAX_DESC_LINES: usize = 2;
    /// Horizontal inset of the text column (left rule + both text pads), matching
    /// the SVG card so the wrap budget agrees with what the renderer draws.
    pub const TEXT_INSET: i32 = 33;

    /// The card height a lifeline needs to fit its content: `has_parent` adds the
    /// dimmed parent line, `desc_lines` the wrapped summary. All cards take the
    /// max across participants so the header row stays flush.
    #[must_use]
    pub fn card_height(has_parent: bool, desc_lines: usize) -> i32 {
        let desc_top = DESC_TOP_Y + if has_parent { DESC_SHIFT_Y } else { 0 };
        let last_baseline = if desc_lines > 0 {
            desc_top + i32::try_from(desc_lines.saturating_sub(1)).unwrap_or(0) * DESC_LINE_H
        } else if has_parent {
            PARENT_Y
        } else {
            TITLE_Y
        };
        (last_baseline + BOTTOM_PAD).max(MIN_H)
    }
}

/// A lifeline head's text below the name: the dimmed parent path
/// (container/component only) and the wrapped, capped summary.
#[derive(Debug, Default)]
struct HeadContent {
    parent_path: Option<String>,
    summary_lines: Vec<String>,
}

/// The head content for every participant, plus the uniform card height that
/// fits the tallest. The text column matches the SVG card's inset, so the wrap
/// budget agrees with what the renderer draws. `parent_path` comes from the
/// projection (the FQN does not carry the C4 ancestry).
fn head_content(
    participants: &[Participant],
    text: &TextMetrics,
    node_w: i32,
) -> (Vec<HeadContent>, i32) {
    let text_w = (node_w - head::TEXT_INSET).max(1);
    let content: Vec<HeadContent> = participants
        .iter()
        .map(|p| HeadContent {
            parent_path: p.parent_path.clone(),
            summary_lines: p
                .summary
                .as_deref()
                .map(|s| text.wrap_desc(s, text_w, head::MAX_DESC_LINES))
                .unwrap_or_default(),
        })
        .collect();
    let node_h = content
        .iter()
        .map(|c| head::card_height(c.parent_path.is_some(), c.summary_lines.len()))
        .fold(head::MIN_H, i32::max);
    (content, node_h)
}

/// The sequence projection engine.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sequence;

impl Projection for Sequence {
    type Input = Diagram;
    type Metrics = Metrics;
    type Output = Layout;

    fn layout(input: &Diagram, metrics: &Metrics) -> Layout {
        layout(input, metrics)
    }
}

/// Lay out `diagram` into absolute coordinates under `metrics`.
#[must_use]
pub fn layout(diagram: &Diagram, metrics: &Metrics) -> Layout {
    let m = metrics;
    if diagram.participants.is_empty() {
        return Layout {
            width: 2 * m.pad,
            height: 2 * m.pad,
            ..Layout::default()
        };
    }

    // Column index per participant id.
    let col: HashMap<&str, usize> = diagram
        .participants
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect();

    // Pass 1 — card width: the widest title, floored at the minimum.
    let node_w = diagram
        .participants
        .iter()
        .map(|p| m.text.title_width(&p.label) + m.card_pad)
        .fold(m.node_w_min, i32::max);

    // Lifeline head content (dimmed parent path + wrapped summary) and the
    // uniform card height that fits the tallest.
    let (content, node_h) = head_content(&diagram.participants, &m.text, node_w);

    // Pass 2 — column gaps: each gap fits only the labels that span it (a wide
    // message widens its own lane, not every lane), plus self-message room,
    // never tighter than card + gutter.
    let base = node_w + m.col_gutter;
    let mut gaps = vec![base; diagram.participants.len().saturating_sub(1)];
    measure(&diagram.items, m, &col, &mut gaps);

    let mut xs = Vec::with_capacity(diagram.participants.len());
    let mut x = m.pad + node_w / 2;
    xs.push(x);
    for &g in &gaps {
        x += g;
        xs.push(x);
    }
    let min_x = xs.iter().copied().min().unwrap_or(m.pad) - m.frame_pad;
    let max_x = xs.iter().copied().max().unwrap_or(m.pad) + m.frame_pad + m.ret_stub;

    // Pass 3 — vertical walk.
    let body_top = m.lifeline_top + node_h + m.msg_gap;
    let mut walk = Walk {
        m,
        col: &col,
        xs: &xs,
        min_x,
        max_x,
        messages: Vec::new(),
        fragments: Vec::new(),
        span: HashMap::new(),
        y: body_top,
        step: 0,
    };
    walk.items(&diagram.items);

    let body_bottom = walk.y;
    // The focus lifeline (accent) is the first participant — the entry's owner,
    // or the trigger actor when one leads the trace.
    let owner = diagram.participants.first().map(|p| p.id.as_str());

    let participants = diagram
        .participants
        .iter()
        .enumerate()
        .map(|(i, p)| PlacedParticipant {
            id: p.id.clone(),
            label: p.label.clone(),
            kind: p.kind.clone(),
            parent_path: content[i].parent_path.clone(),
            summary_lines: content[i].summary_lines.clone(),
            card: Rect::new(xs[i] - node_w / 2, m.lifeline_top, node_w, node_h),
            lifeline_x: xs[i],
            top: m.lifeline_top + node_h,
            bottom: body_bottom,
        })
        .collect();

    let activations = diagram
        .participants
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            walk.span.get(p.id.as_str()).map(|&(lo, hi)| Activation {
                participant: p.id.clone(),
                x: xs[i],
                top: lo - 6,
                bottom: hi + 6,
                owner: Some(p.id.as_str()) == owner,
            })
        })
        .collect();

    // Right extent: the rightmost card edge, fragment border, or self label.
    let mut right = xs.last().map_or(m.pad, |&x| x + node_w / 2);
    for f in &walk.fragments {
        right = right.max(f.rect.right());
    }
    for msg in &walk.messages {
        if msg.kind == MsgKind::SelfMsg {
            right = right.max(msg.from_x + m.self_offset + m.text.label_width(&msg.label));
        }
    }

    Layout {
        width: right + m.pad,
        height: body_bottom + m.pad,
        participants,
        messages: walk.messages,
        activations,
        fragments: walk.fragments,
    }
}

/// Widen the inter-lifeline gaps a diagram needs: each call/return label must
/// fit the gaps it spans (its width shared evenly across them), and a
/// self-message reserves room in the gap to its right. A gap no wide label
/// spans keeps its card + gutter floor — one long label no longer spreads every
/// column. `gaps[k]` is the gap between columns `k` and `k + 1`.
fn measure(items: &[Item], m: &Metrics, col: &HashMap<&str, usize>, gaps: &mut [i32]) {
    for item in items {
        match item {
            Item::Message(msg) => match msg.kind {
                MsgKind::SelfMsg => {
                    // The loop extends right of its lifeline; reserve that room
                    // in the gap to the next column. On the last column it runs
                    // into the canvas margin, sized by the right-extent pass.
                    let c = col.get(msg.from.as_str()).copied().unwrap_or(0);
                    if let Some(g) = gaps.get_mut(c) {
                        *g = (*g).max(m.self_offset + m.text.label_width(&msg.label) + 24);
                    }
                }
                MsgKind::Call | MsgKind::Return => {
                    let a = col.get(msg.from.as_str()).copied().unwrap_or(0);
                    let b = col.get(msg.to.as_str()).copied().unwrap_or(0);
                    let (lo, hi) = (a.min(b), a.max(b));
                    let span = i32::try_from(hi - lo).unwrap_or(1).max(1);
                    // Clear the activation bars at both endpoints (each `act_w / 2`
                    // off its lifeline) plus breathing room, so a centred label's
                    // backing pill never butts against a lifeline's activation.
                    let need = m.message_pill_width(msg) + m.act_w + 16;
                    let per = (need + span - 1) / span; // ceil(need / span)
                    for g in &mut gaps[lo..hi] {
                        *g = (*g).max(per);
                    }
                }
            },
            Item::Fragment(frag) => {
                for section in &frag.sections {
                    measure(&section.body, m, col, gaps);
                }
            }
        }
    }
}

/// Mutable state threaded through the vertical walk.
struct Walk<'a> {
    m: &'a Metrics,
    col: &'a HashMap<&'a str, usize>,
    xs: &'a [i32],
    min_x: i32,
    max_x: i32,
    messages: Vec<PlacedMessage>,
    fragments: Vec<PlacedFragment>,
    span: HashMap<&'a str, (i32, i32)>,
    y: i32,
    step: u32,
}

impl<'a> Walk<'a> {
    fn x_of(&self, id: &str) -> Option<i32> {
        self.col.get(id).map(|&i| self.xs[i])
    }

    /// Record that lifeline `id` is involved at `y`, growing its activation span.
    fn note(&mut self, id: &'a str, y: i32) {
        self.span
            .entry(id)
            .and_modify(|(lo, hi)| {
                *lo = (*lo).min(y);
                *hi = (*hi).max(y);
            })
            .or_insert((y, y));
    }

    fn items(&mut self, items: &'a [Item]) {
        for item in items {
            match item {
                Item::Message(msg) => self.message(msg),
                Item::Fragment(frag) => self.fragment(frag),
            }
        }
    }

    fn message(&mut self, msg: &'a Message) {
        let m = self.m;
        let from_x = self.x_of(&msg.from).unwrap_or(self.min_x);
        match msg.kind {
            MsgKind::SelfMsg => {
                let y = self.y;
                self.step += 1;
                self.note(msg.from.as_str(), y);
                self.messages.push(PlacedMessage {
                    kind: MsgKind::SelfMsg,
                    from: msg.from.clone(),
                    to: msg.to.clone(),
                    from_x,
                    to_x: from_x,
                    y,
                    dir: 1,
                    step: Some(self.step),
                    label: msg.label.clone(),
                    detail: msg.detail.clone(),
                });
                self.y += m.msg_gap + m.self_extra;
            }
            MsgKind::Call | MsgKind::Return => {
                let to_x = self.x_of(&msg.to).unwrap_or(from_x - m.ret_stub);
                let dir = if to_x >= from_x { 1 } else { -1 };
                let y = self.y;
                let step = if msg.kind == MsgKind::Call {
                    self.step += 1;
                    Some(self.step)
                } else {
                    None
                };
                self.note(msg.from.as_str(), y);
                self.note(msg.to.as_str(), y);
                self.messages.push(PlacedMessage {
                    kind: msg.kind,
                    from: msg.from.clone(),
                    to: msg.to.clone(),
                    from_x: from_x + dir * (m.act_w / 2),
                    to_x: to_x - dir * (m.act_w / 2),
                    y,
                    dir,
                    step,
                    label: msg.label.clone(),
                    detail: msg.detail.clone(),
                });
                self.y += m.msg_gap;
            }
        }
    }

    fn fragment(&mut self, frag: &'a Fragment) {
        let m = self.m;
        let top = self.y;
        self.y += m.frame_head;
        let label = frag
            .sections
            .first()
            .map(|s| s.guard.clone())
            .unwrap_or_default();
        let mut dividers = Vec::new();
        for (idx, section) in frag.sections.iter().enumerate() {
            if idx > 0 {
                let div_y = self.y + m.else_pad;
                dividers.push(Divider {
                    y: div_y,
                    guard: if section.guard.is_empty() {
                        "else".to_owned()
                    } else {
                        section.guard.clone()
                    },
                });
                self.y = div_y + m.else_head;
            }
            self.items(&section.body);
        }
        self.y += m.frame_foot;
        // The box spans only the lifelines the fragment touches, padded — so its
        // operator tab and guard sit beside the involved lifelines, not off at
        // the leftmost participant.
        let (lo, hi) = self.fragment_extent(&frag.sections);
        let rect = Rect::new(
            lo - m.frame_pad,
            top,
            (hi - lo) + 2 * m.frame_pad,
            self.y - top,
        );
        self.fragments.push(PlacedFragment {
            kind: frag.kind,
            rect,
            label,
            dividers,
        });
        self.y += m.frame_gap;
    }

    /// The horizontal extent (`min_x`, `max_x`) of the lifelines a fragment's
    /// messages touch, recursing into nested fragments. A return to a
    /// non-participant (the omitted caller) reaches a left stub, so its sender's
    /// lifeline minus a return stub bounds the left edge. Empty fragments fall
    /// back to the full width.
    fn fragment_extent(&self, sections: &[Section]) -> (i32, i32) {
        let mut bounds = Bounds::new();
        for section in sections {
            self.collect_extent(&section.body, &mut bounds);
        }
        bounds
            .rect()
            .map_or((self.min_x, self.max_x), |r| (r.x, r.right()))
    }

    fn collect_extent(&self, items: &[Item], bounds: &mut Bounds) {
        for item in items {
            match item {
                Item::Message(msg) => {
                    let Some(from_x) = self.x_of(&msg.from) else {
                        continue;
                    };
                    bounds.include(from_x, 0);
                    match msg.kind {
                        MsgKind::SelfMsg => bounds.include(from_x + self.m.self_offset, 0),
                        MsgKind::Call | MsgKind::Return => {
                            let to_x = self.x_of(&msg.to).unwrap_or(from_x - self.m.ret_stub);
                            bounds.include(to_x, 0);
                        }
                    }
                }
                Item::Fragment(frag) => {
                    for section in &frag.sections {
                        self.collect_extent(&section.body, bounds);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str) -> Participant {
        Participant {
            id: id.to_owned(),
            label: id.to_owned(),
            kind: "component".to_owned(),
            summary: None,
            parent_path: None,
        }
    }

    fn call(from: &str, to: &str, label: &str) -> Item {
        Item::Message(Message {
            from: from.to_owned(),
            to: to.to_owned(),
            kind: MsgKind::Call,
            label: label.to_owned(),
            detail: String::new(),
        })
    }

    fn ret(from: &str, to: &str, marker: &str) -> Item {
        Item::Message(Message {
            from: from.to_owned(),
            to: to.to_owned(),
            kind: MsgKind::Return,
            label: marker.to_owned(),
            detail: String::new(),
        })
    }

    #[test]
    fn empty_diagram_is_minimal() {
        let out = layout(&Diagram::default(), &Metrics::default());
        assert!(out.participants.is_empty());
        assert!(out.width > 0 && out.height > 0);
    }

    #[test]
    fn component_lifeline_carries_parent_path_and_grows_card() {
        let d = Diagram {
            participants: vec![Participant {
                id: "main::Validator".to_owned(),
                label: "Validator".to_owned(),
                kind: "component".to_owned(),
                summary: Some("Checks the order is well formed before it is queued.".to_owned()),
                parent_path: Some("shop::api".to_owned()),
            }],
            items: vec![],
        };
        let out = layout(&d, &Metrics::default());
        let p = &out.participants[0];
        assert_eq!(p.parent_path.as_deref(), Some("shop::api"));
        assert!(!p.summary_lines.is_empty() && p.summary_lines.len() <= head::MAX_DESC_LINES);
        // The card grew past the floor to fit the parent line + summary, and the
        // dashed lifeline starts at the (taller) card's bottom.
        assert!(p.card.h > head::MIN_H);
        assert_eq!(p.top, Metrics::default().lifeline_top + p.card.h);
    }

    #[test]
    fn person_lifeline_has_no_parent_path_and_keeps_floor_height() {
        let d = Diagram {
            participants: vec![Participant {
                id: "main::Customer".to_owned(),
                label: "Customer".to_owned(),
                kind: "person".to_owned(),
                summary: None,
                parent_path: None,
            }],
            items: vec![],
        };
        let out = layout(&d, &Metrics::default());
        assert_eq!(out.participants[0].parent_path, None);
        assert_eq!(out.participants[0].card.h, head::MIN_H);
    }

    #[test]
    fn call_then_return_advances_and_numbers() {
        let d = Diagram {
            participants: vec![p("A"), p("B")],
            items: vec![call("A", "B", "Do"), ret("B", "A", "Ok")],
        };
        let out = layout(&d, &Metrics::default());
        assert_eq!(out.messages.len(), 2);
        assert_eq!(out.messages[0].step, Some(1)); // call numbered
        assert_eq!(out.messages[1].step, None); // return not numbered
        assert!(out.messages[1].y > out.messages[0].y); // advances downward
    }

    #[test]
    fn alt_fragment_splits_with_divider_between_sections() {
        let frag = Item::Fragment(Fragment {
            kind: FragKind::Alt,
            sections: vec![
                Section {
                    guard: "c".to_owned(),
                    body: vec![ret("B", "A", "Ok")],
                },
                Section {
                    guard: String::new(),
                    body: vec![ret("B", "A", "Err")],
                },
            ],
        });
        let d = Diagram {
            participants: vec![p("A"), p("B")],
            items: vec![call("A", "B", "Do"), frag],
        };
        let out = layout(&d, &Metrics::default());
        let f = &out.fragments[0];
        assert_eq!(f.dividers.len(), 1);
        assert_eq!(f.label, "c");
        assert_eq!(f.dividers[0].guard, "else");
        // The two returns straddle the divider, and the box encloses both.
        let ok_y = out.messages.iter().find(|m| m.label == "Ok").unwrap().y;
        let err_y = out.messages.iter().find(|m| m.label == "Err").unwrap().y;
        assert!(ok_y < f.dividers[0].y && f.dividers[0].y < err_y);
        assert!(f.rect.y <= ok_y && err_y <= f.rect.bottom());
    }

    #[test]
    fn a_wide_label_widens_only_its_own_lane() {
        let m = Metrics::default();
        let node_w = m.node_w_min; // single-char titles stay at the floor
        let base = node_w + m.col_gutter;
        let wide = "a_very_long_descriptive_method_name_that_needs_room";
        let d = Diagram {
            participants: vec![p("A"), p("B"), p("C"), p("D")],
            items: vec![
                call("A", "B", "Do"),
                call("B", "C", wide),
                call("C", "D", "Do"),
            ],
        };
        let xs: Vec<i32> = layout(&d, &m)
            .participants
            .iter()
            .map(|q| q.lifeline_x)
            .collect();
        // Lanes the wide label does not span keep the card+gutter floor; only the
        // B–C lane widens, and by enough to fit the label.
        assert_eq!(xs[1] - xs[0], base, "A–B lane should stay at the floor");
        assert_eq!(xs[3] - xs[2], base, "C–D lane should stay at the floor");
        assert!(
            xs[2] - xs[1] >= m.text.label_width(wide),
            "B–C lane must fit the wide label"
        );
    }
}
