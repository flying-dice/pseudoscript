//! The prebuilt Svelte bundles, embedded at compile time.
//!
//! `build.rs` guarantees these files exist (with a regeneration hint when they
//! do not), so the `include_str!`s below always resolve in a checked-out tree.

/// The Svelte SSR bundle: an IIFE exposing `globalThis.SSR.renderPage`.
pub(crate) const SSR_JS: &str = include_str!("assets/ssr.js");

/// The client progressive-enhancement bundle (pan/zoom, search palette, theme
/// toggle, drawer, copy buttons) — no hydration.
pub(crate) const CLIENT_JS: &str = include_str!("assets/client.js");

/// The universe page's 3D island bundle (three.js + d3-force-3d); linked only
/// by `universe.html`.
pub(crate) const UNIVERSE_JS: &str = include_str!("assets/universe.js");

/// The site stylesheet (design tokens, layout, figures, the --pds-* diagram
/// palette per theme).
pub(crate) const STYLE_CSS: &str = include_str!("assets/style.css");
