//! Diagram emission for `PseudoScript` (`LANG.md` §9, ADR-017).
//!
//! This crate sits above [`pseudoscript_model`]: it projects diagram *views*
//! from the resolved model [`Graph`](pseudoscript_model::Graph) into a [`Scene`]
//! IR — laid-out, notation-neutral geometry — and renders a `Scene` to SVG.
//! The SVGs embed in the `pds doc` site (ADR-017); the `Scene` IR is the
//! conformance surface (`CONFORMANCE/generation/README.md`), pinned by
//! [`Scene::to_golden`].
//!
//! It is WASM-safe: no threads, filesystem, clock, or native dependencies. SVG
//! is built with [`std::fmt::Write`] string-building — no template engine, no
//! headless browser.
//!
//! # Surfaces
//!
//! - [`project`] — a [`View`]-keyed strategy projecting a [`Scene`] from a
//!   [`Graph`](pseudoscript_model::Graph). Returns [`EmitError`] when a view's
//!   target FQN does not resolve to the required node kind.
//! - [`Scene`] (and its sub-types) — the serde-serialisable IR; [`Scene::to_golden`]
//!   reproduces the `CONFORMANCE/generation` text format.
//! - [`render_svg`] — renders a laid-out [`Scene`] to a self-contained SVG
//!   document.
//! - [`graph_of_source`] — convenience: build the [`Graph`](pseudoscript_model::Graph)
//!   for a single-file generation fixture, deriving its module FQN from the
//!   `//!` inner doc.
//!
//! # Example
//!
//! ```
//! use pseudoscript_emit::{graph_of_source, project, render_svg, View};
//!
//! let graph = graph_of_source("//! shop\npublic person Customer;\npublic system Shop;");
//! let scene = project(&graph, View::Context).expect("context view");
//! assert!(scene.to_golden().contains("node shop::Customer person \"Customer\""));
//! assert!(render_svg(&scene).starts_with("<svg"));
//! ```

mod c4_render;
mod project;
mod render;
mod scene;

pub use c4_render::{
    BoundaryFrame, C4Layout, C4Tweaks, GridInfo, GridPin, GridSearch, LaidOutEdge, LaidOutNode,
    PointI, layout_c4_scene, layout_c4_scene_with,
};
pub use project::{EmitError, View, project, project_symbol};
pub use render::{
    Theme, adaptive_style_block, layout_data_scene, layout_feature_scene, layout_sequence_scene,
    render_svg, render_svg_themed,
};
pub use scene::{
    C4EdgeKind, C4Scene, C4View, DataEntity, DataLink, DataScene, EntityForm, EntityRow,
    FeatureScene, FeatureStepNode, Frame, FrameKind, Lifeline, Message, MessageKind, PlacedNode,
    Rect, RoutedEdge, Scene, SeqItem, SequenceScene,
};

use pseudoscript_model::{Graph, WorkspaceModule, graph};

/// Builds the resolved [`Graph`](pseudoscript_model::Graph) for a single-file
/// generation fixture, deriving its module FQN from the first `//!` inner-doc
/// token (matching the model's single-file `check`).
#[must_use]
pub fn graph_of_source(source: &str) -> Graph {
    let fqn = module_fqn(source);
    graph(&[WorkspaceModule::new(fqn, source.to_owned())])
}

/// The module FQN from a source's first `//!` inner-doc token.
fn module_fqn(source: &str) -> String {
    source
        .lines()
        .find_map(|line| line.trim().strip_prefix("//!"))
        .and_then(|doc| doc.split_whitespace().next())
        .unwrap_or("")
        .to_owned()
}
