//! The prebuilt Svelte bundles, embedded at compile time.
//!
//! `build.rs` guarantees these files exist (with a regeneration hint when they
//! do not), so the `include_str!`s below always resolve in a checked-out tree.

/// The Svelte SSR bundle: an IIFE exposing `globalThis.SSR.renderPage`.
pub(crate) const SSR_JS: &str = include_str!("assets/ssr.js");

/// The client hydration + diagram-island bundle.
pub(crate) const CLIENT_JS: &str = include_str!("assets/client.js");

/// The site stylesheet (design tokens, layout, Svelte Flow, timeline).
pub(crate) const STYLE_CSS: &str = include_str!("assets/style.css");
