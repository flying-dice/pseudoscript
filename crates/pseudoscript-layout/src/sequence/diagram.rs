//! The sequence-diagram input IR — what a projection hands the layout engine.
//! Purely structural: participants and an ordered tree of messages and
//! combined-fragments. Carries no coordinates; positioning is the engine's job.

use serde::{Deserialize, Serialize};

/// A complete sequence diagram to lay out.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Diagram {
    /// Lifelines, left to right in first-appearance order.
    pub participants: Vec<Participant>,
    /// Messages and fragments, in evaluation order.
    pub items: Vec<Item>,
}

/// A lifeline: a stable `id` the messages reference, a display `label`, and a
/// `kind` token (the C4 kind, used only for head-card styling).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub label: String,
    pub kind: String,
}

/// One ordered element: a message or a nestable fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Item {
    Message(Message),
    Fragment(Fragment),
}

/// A message between lifelines (or a self-message).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Message {
    /// Source lifeline id.
    pub from: String,
    /// Target lifeline id (equals `from` for a self-message).
    pub to: String,
    pub kind: MsgKind,
    /// Primary label (method name, or a return marker such as `Ok`/`Err`).
    pub label: String,
    /// Dimmed detail after the label (a call signature, or a return type).
    #[serde(default)]
    pub detail: String,
}

/// The kind of a message, which drives its glyph and vertical advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MsgKind {
    /// A solid call to another lifeline.
    #[default]
    Call,
    /// A dashed return to the caller.
    Return,
    /// A loop on the sender's own lifeline.
    #[serde(rename = "self")]
    SelfMsg,
}

/// A combined fragment (`alt`/`loop`) over one or more sections. An `alt` has a
/// `then` section and, optionally, an `else` section; the layout engine splits
/// the box with a divider between consecutive sections.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Fragment {
    pub kind: FragKind,
    /// At least one; sections after the first render below a divider.
    pub sections: Vec<Section>,
}

/// One compartment of a fragment: its guard label and its body.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Section {
    /// The guard shown in the operator tab (first section) or beside the
    /// divider (later sections). May be empty.
    pub guard: String,
    pub body: Vec<Item>,
}

/// The kind of fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FragKind {
    #[default]
    Alt,
    Loop,
}

impl FragKind {
    /// The operator keyword drawn in the fragment tab.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            FragKind::Alt => "alt",
            FragKind::Loop => "loop",
        }
    }
}
