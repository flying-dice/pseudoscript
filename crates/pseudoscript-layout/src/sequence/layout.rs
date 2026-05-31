//! The positioned output of the sequence engine — absolute renderer
//! coordinates a consumer draws verbatim. Serde-serializable so it crosses the
//! wasm boundary unchanged.

use serde::{Deserialize, Serialize};

use crate::geom::Rect;
use crate::sequence::diagram::{FragKind, MsgKind};

/// A fully positioned sequence diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Layout {
    /// Total canvas width.
    pub width: i32,
    /// Total canvas height.
    pub height: i32,
    pub participants: Vec<PlacedParticipant>,
    pub messages: Vec<PlacedMessage>,
    pub activations: Vec<Activation>,
    pub fragments: Vec<PlacedFragment>,
}

/// A lifeline, with its head card and the vertical span of its dashed line.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlacedParticipant {
    pub id: String,
    pub label: String,
    pub kind: String,
    /// The head card rectangle.
    pub card: Rect,
    /// The lifeline's centre x (the dashed line and activations sit here).
    pub lifeline_x: i32,
    /// Where the dashed lifeline starts (bottom of the card).
    pub top: i32,
    /// Where the dashed lifeline ends.
    pub bottom: i32,
}

/// A positioned message: an arrow at `y` from `from_x` to `to_x`. For a
/// self-message `from_x == to_x` and the renderer draws the loop to the right.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlacedMessage {
    pub kind: MsgKind,
    /// Source/target lifeline ids (carried through for the renderer/tests).
    pub from: String,
    pub to: String,
    pub from_x: i32,
    pub to_x: i32,
    pub y: i32,
    /// `+1` left-to-right, `-1` right-to-left.
    pub dir: i32,
    /// Reading-order step number (calls and self-messages only).
    pub step: Option<u32>,
    pub label: String,
    pub detail: String,
}

/// An execution-activation bar on a lifeline, spanning the participant's
/// first-to-last involvement.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Activation {
    pub participant: String,
    pub x: i32,
    pub top: i32,
    pub bottom: i32,
    /// The entry's owner lifeline (the focus), styled with the accent.
    pub owner: bool,
}

/// A positioned combined fragment: its box, operator label, and the dividers
/// splitting its sections.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlacedFragment {
    pub kind: FragKind,
    pub rect: Rect,
    /// The first section's guard, shown in the operator tab.
    pub label: String,
    /// One per section after the first, top to bottom.
    pub dividers: Vec<Divider>,
}

/// A horizontal split between two fragment sections.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Divider {
    pub y: i32,
    /// The following section's guard (`else`, or an explicit condition).
    pub guard: String,
}
