//! The static site assets: `style.css` and `app.js`.
//!
//! These ship once at the site root and every page links them, rather than
//! inlining them per page (`LANG.md` §9.3). The CSS carries both the light and
//! dark variable sets, switched by the `data-theme` attribute on `<html>`; the
//! JS is dependency-free vanilla — a collapsible sidebar tree, a client-side
//! filter over the tree, and wheel/drag zoom-pan on each inline diagram.

/// The site stylesheet. Clean-technical, cargo-doc-like: a fixed left sidebar
/// tree, a neutral palette, a system font stack, and diagrams framed as figures.
pub const STYLE_CSS: &str = include_str!("assets/style.css");

/// The site script: collapsible tree, search filter, and diagram zoom-pan.
pub const APP_JS: &str = include_str!("assets/app.js");
