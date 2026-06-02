//! The `Scene` IR: laid-out, notation-neutral diagram geometry.
//!
//! A [`Scene`] is what a [`crate::project`]ion of the resolved graph produces and
//! what [`crate::render_svg`] turns into pixels. It is the conformance surface
//! (`CONFORMANCE/generation/README.md`): [`Scene::to_golden`] serialises it to
//! the exact text format the `.scene` goldens pin — coordinates omitted, in
//! canonical order. Coordinates are carried internally for the renderer only.

use std::fmt::Write as _;

use pseudoscript_model::NodeKind;
use serde::{Deserialize, Serialize};

/// A laid-out diagram: a C4 view (placed nodes + routed edges) or a sequence
/// view (lifelines + messages + frames).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "view")]
pub enum Scene {
    /// A C4 context/container/component view (`LANG.md` §9.1).
    C4(C4Scene),
    /// A sequence view (`LANG.md` §9.2).
    Sequence(SequenceScene),
}

/// Which C4 view a [`C4Scene`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum C4View {
    /// Persons and systems (`LANG.md` §9.1).
    Context,
    /// One system's containers.
    Container,
    /// One container's components.
    Component,
}

impl C4View {
    /// The `view` keyword this view writes in the golden header.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            C4View::Context => "context",
            C4View::Container => "container",
            C4View::Component => "component",
        }
    }
}

/// A laid-out C4 view: an ordered set of placed nodes and routed edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4Scene {
    /// Which C4 view this is.
    pub view: C4View,
    /// The view's boundary node FQN (`of`): the system for a container view, the
    /// container for a component view. `None` for context.
    pub of: Option<String>,
    /// Placed nodes, in source-declaration order.
    pub nodes: Vec<PlacedNode>,
    /// Routed edges, sorted by `(from, to, kind, label)`.
    pub edges: Vec<RoutedEdge>,
}

/// A node placed in a C4 view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedNode {
    /// The node's fully-qualified name.
    pub fqn: String,
    /// The node's C4 kind (`person`/`system`/`container`/`component`).
    pub kind: NodeKind,
    /// The display label: the node's simple name.
    pub label: String,
    /// The node's `///` summary, when it has one; rendered as the card's dimmed
    /// description. Carried for the renderer only — never serialised by
    /// [`Scene::to_golden`].
    pub summary: Option<String>,
    /// The boundary FQN this node sits inside, when inside the view's boundary.
    pub boundary: Option<String>,
    /// Layout rectangle, for the renderer.
    pub rect: Rect,
}

/// The relationship a [`RoutedEdge`] expresses in a C4 view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum C4EdgeKind {
    /// A body call between nodes.
    Call,
    /// A synthesised trigger initiator → triggered node.
    Trigger,
    /// A `from` composition provenance edge.
    Provenance,
}

impl C4EdgeKind {
    /// The keyword this edge writes in the golden.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            C4EdgeKind::Call => "call",
            C4EdgeKind::Trigger => "trigger",
            C4EdgeKind::Provenance => "provenance",
        }
    }
}

/// An edge routed between two C4 nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEdge {
    /// Source endpoint FQN.
    pub from: String,
    /// Target endpoint FQN.
    pub to: String,
    /// The relationship kind.
    pub kind: C4EdgeKind,
    /// Edge label (the method name for a call, else empty).
    pub label: String,
}

/// A laid-out sequence view: lifelines, messages, and nested frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceScene {
    /// The triggered callable the trace starts from.
    pub entry: String,
    /// Lifelines, in order of first appearance.
    pub participants: Vec<Lifeline>,
    /// Messages and frames, in body evaluation order.
    pub items: Vec<SeqItem>,
}

/// A sequence lifeline (a participant).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lifeline {
    /// The participant's FQN.
    pub fqn: String,
    /// The participant node's C4 kind, for the lifeline-head card styling.
    pub kind: NodeKind,
    /// The node's `///` summary, shown dimmed under the name (like a C4 card).
    /// `None` for synthesised initiators and unresolved targets.
    #[serde(default)]
    pub summary: Option<String>,
    /// The structural ancestry shown dimmed under a container/component name
    /// (enclosing node names, outermost first, joined with `::`). The FQN is
    /// module-flat, so this is derived from the graph, not the FQN. `None` for
    /// other kinds and top-level nodes.
    #[serde(default)]
    pub parent_path: Option<String>,
}

/// One ordered item in a sequence trace: a message or a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SeqItem {
    /// A message between (or within) lifelines.
    Message(Message),
    /// A nestable `alt`/`loop` frame over a body of items.
    Frame(Frame),
}

/// The kind of a sequence message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageKind {
    /// A call to another lifeline.
    Call,
    /// A return to the caller.
    Return,
    /// A self-message (owner to owner).
    #[serde(rename = "self")]
    SelfMsg,
}

impl MessageKind {
    /// The keyword this message writes in the golden.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            MessageKind::Call => "call",
            MessageKind::Return => "return",
            MessageKind::SelfMsg => "self",
        }
    }
}

/// A message between two lifelines (or a self-message).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Source lifeline FQN.
    pub from: String,
    /// Target lifeline FQN.
    pub to: String,
    /// The message kind.
    pub kind: MessageKind,
    /// The message label (method name, or `Ok`/`Err`/empty for a return).
    pub label: String,
    /// The type detail shown after the label — a call's `(params): ret`
    /// signature, or a return's concrete type. Carried for the renderer only;
    /// never serialised by [`Scene::to_golden`]. Empty when unknown.
    pub detail: String,
}

/// The kind of a sequence frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameKind {
    /// An `if`/`else` → `alt` frame.
    Alt,
    /// A `for`/`while` → `loop` frame.
    Loop,
}

impl FrameKind {
    /// The keyword this frame writes in the golden.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            FrameKind::Alt => "alt",
            FrameKind::Loop => "loop",
        }
    }
}

/// A nestable frame over a body of sequence items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// `alt` or `loop`.
    pub kind: FrameKind,
    /// The frame's condition label.
    pub cond: String,
    /// The framed body items, indented one level under the frame in the golden.
    pub body: Vec<SeqItem>,
}

/// An axis-aligned layout rectangle (renderer coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl Scene {
    /// Serialises the scene to the `CONFORMANCE/generation/README.md` golden
    /// text format: one element per line, UTF-8, `\n`-terminated, in canonical
    /// order, coordinates omitted.
    #[must_use]
    pub fn to_golden(&self) -> String {
        let mut out = String::new();
        match self {
            Scene::C4(scene) => scene.write_golden(&mut out),
            Scene::Sequence(scene) => scene.write_golden(&mut out),
        }
        out
    }
}

impl C4Scene {
    fn write_golden(&self, out: &mut String) {
        let _ = writeln!(out, "view {}", self.view.keyword());
        if let Some(of) = &self.of {
            let _ = writeln!(out, "of {of}");
        }
        for node in &self.nodes {
            let _ = write!(
                out,
                "node {} {} {}",
                node.fqn,
                node.kind.keyword(),
                quote(&node.label),
            );
            if let Some(boundary) = &node.boundary {
                let _ = write!(out, " in {boundary}");
            }
            out.push('\n');
        }
        for edge in &self.edges {
            let _ = writeln!(
                out,
                "edge {} -> {} {} {}",
                edge.from,
                edge.to,
                edge.kind.keyword(),
                quote(&edge.label),
            );
        }
    }
}

impl SequenceScene {
    fn write_golden(&self, out: &mut String) {
        let _ = writeln!(out, "view sequence");
        let _ = writeln!(out, "entry {}", self.entry);
        for participant in &self.participants {
            let _ = writeln!(out, "participant {}", participant.fqn);
        }
        write_items(&self.items, 0, out);
    }
}

/// Writes a sequence-item body, frames indenting two spaces per nesting level.
fn write_items(items: &[SeqItem], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    for item in items {
        match item {
            SeqItem::Message(msg) => {
                let _ = writeln!(
                    out,
                    "{pad}message {} -> {} {} {}",
                    msg.from,
                    msg.to,
                    msg.kind.keyword(),
                    quote(&msg.label),
                );
            }
            SeqItem::Frame(frame) => {
                let _ = writeln!(
                    out,
                    "{pad}frame {} {}",
                    frame.kind.keyword(),
                    quote(&frame.cond),
                );
                write_items(&frame.body, indent + 1, out);
            }
        }
    }
}

/// Quotes a label, escaping `\` and `"` (golden lexeme rule).
fn quote(label: &str) -> String {
    let mut out = String::with_capacity(label.len() + 2);
    out.push('"');
    for ch in label.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_backslash_and_quote() {
        assert_eq!(quote(r#"a\b"c"#), r#""a\\b\"c""#);
    }

    #[test]
    fn context_golden_has_no_of_line() {
        let scene = Scene::C4(C4Scene {
            view: C4View::Context,
            of: None,
            nodes: vec![PlacedNode {
                fqn: "m::A".to_owned(),
                kind: NodeKind::System,
                label: "A".to_owned(),
                summary: None,
                boundary: None,
                rect: Rect::default(),
            }],
            edges: Vec::new(),
        });
        assert_eq!(scene.to_golden(), "view context\nnode m::A system \"A\"\n");
    }
}
