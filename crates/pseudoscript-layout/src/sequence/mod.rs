//! The sequence-diagram layout engine.
//!
//! Implements the standard sequence-diagram layout pattern (as used by the
//! mainstream open-source tools) in three passes:
//!
//! 1. **Measure** — participant card widths from their titles, and per-message
//!    label widths from the rendered text.
//! 2. **Space columns** — pick a uniform inter-lifeline gap wide enough that
//!    every message label fits the lane it spans and no card overlaps its
//!    neighbour, then place lifelines.
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
    /// Participant card height.
    pub node_h: i32,
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
            node_h: 60,
            lifeline_top: 64,
            msg_gap: 54,
            self_extra: 26,
            frame_head: 46,
            frame_foot: 14,
            frame_gap: 20,
            else_pad: 12,
            else_head: 30,
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

    // Pass 2 — column gap: fit the widest label per lane it spans, plus
    // self-message room, never tighter than card + gutter.
    let mut need_gap = 0;
    let mut self_room = 0;
    measure(&diagram.items, m, &col, &mut need_gap, &mut self_room);
    let gap = (node_w + m.col_gutter).max(need_gap).max(self_room);

    let xs: Vec<i32> = (0..diagram.participants.len())
        .map(|i| m.pad + node_w / 2 + i32::try_from(i).unwrap_or(0) * gap)
        .collect();
    let min_x = xs.iter().copied().min().unwrap_or(m.pad) - m.frame_pad;
    let max_x = xs.iter().copied().max().unwrap_or(m.pad) + m.frame_pad + m.ret_stub;

    // Pass 3 — vertical walk.
    let body_top = m.lifeline_top + m.node_h + m.msg_gap;
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
            card: Rect::new(xs[i] - node_w / 2, m.lifeline_top, node_w, m.node_h),
            lifeline_x: xs[i],
            top: m.lifeline_top + m.node_h,
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

/// Accumulate the column gap a diagram needs: per inter-lifeline message, the
/// label must fit the columns it spans; self-messages reserve room to the right.
fn measure(
    items: &[Item],
    m: &Metrics,
    col: &HashMap<&str, usize>,
    need_gap: &mut i32,
    self_room: &mut i32,
) {
    for item in items {
        match item {
            Item::Message(msg) => match msg.kind {
                MsgKind::SelfMsg => {
                    *self_room =
                        (*self_room).max(m.self_offset + m.text.label_width(&msg.label) + 24);
                }
                MsgKind::Call | MsgKind::Return => {
                    let a = i32::try_from(col.get(msg.from.as_str()).copied().unwrap_or(0))
                        .unwrap_or(0);
                    let b =
                        i32::try_from(col.get(msg.to.as_str()).copied().unwrap_or(0)).unwrap_or(0);
                    let span = (a - b).abs().max(1);
                    let w = m.message_pill_width(msg);
                    *need_gap = (*need_gap).max((w + span - 1) / span); // ceil(w / span)
                }
            },
            Item::Fragment(frag) => {
                for section in &frag.sections {
                    measure(&section.body, m, col, need_gap, self_room);
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
}
