//! The `PseudoScript` documentation site renderer (`LANG.md` §9.3, ADR-017).
//!
//! The presentation is authored in Svelte and rendered through an embedded
//! `QuickJS` engine ([`rquickjs`]) at generation time, so the shipped binary
//! needs no JavaScript toolchain — only the prebuilt bundles embedded into it.
//! The diagrams are interactive client islands: C4 views render as a Svelte Flow
//! graph, and sequence views as an animated code-flow timeline. The server-side
//! pass renders the page chrome and all text content for first paint.
//!
//! The model→page logic — the [`DocConfig`]/[`Theme`] config, the URL scheme,
//! graph navigation, HTML [`escape`]ing, and the [`Site`] / [`SiteFile`] value
//! types — lives in this crate and is consumed by the SSR props builder.
//!
//! # Surfaces
//!
//! - [`try_render_site_with`] — the host-agnostic core: a resolved graph, a
//!   [`DocConfig`], and any [`SsrEngine`] → a [`Site`]. The only entry available
//!   on every target (a wasm host supplies its own engine).
//! - [`render_site`] / [`try_render_site`] — native convenience entries that use
//!   the embedded `QuickJS` engine ([`QuickJsEngine`]); gated out on wasm, where
//!   `rquickjs` (which compiles `QuickJS` C) does not build.

mod assets;
mod config;
mod escape;
mod nav;
mod props;
mod render;
mod render_md;
mod renderer;
mod shell;
mod site;
mod url;

pub use config::{DocConfig, DocGroup, DocPage, Theme};
pub use escape::escape;
pub use props::RenderedPage;
pub use renderer::{RenderError, SsrEngine};
pub use site::{Site, SiteFile};

#[cfg(not(target_arch = "wasm32"))]
pub use renderer::QuickJsEngine;

use pseudoscript_model::Graph;

/// The Svelte SSR bundle (`ssr.js`) embedded in the binary, an IIFE defining
/// `globalThis.SSR.renderPage(propsJson) -> {head, body}` JSON. A wasm host
/// loads this into its own JavaScript engine to back an [`SsrEngine`]; the
/// native [`QuickJsEngine`] evaluates it internally.
#[must_use]
pub fn ssr_bundle() -> &'static str {
    assets::SSR_JS
}

/// Renders the whole documentation site for `graph` under `config`, driving SSR
/// through `engine`. Host-agnostic: the same `ssr.js` bundle runs under the
/// native [`QuickJsEngine`] or any wasm-host engine implementing [`SsrEngine`].
///
/// # Errors
///
/// Returns [`RenderError`] when a page fails to serialise, the engine call
/// fails, or its result cannot be decoded.
pub fn try_render_site_with(
    graph: &Graph,
    config: &DocConfig,
    engine: &impl SsrEngine,
) -> Result<Site, RenderError> {
    let pages = render::build_pages(graph, config);

    let mut files = Vec::with_capacity(pages.len() + 2);
    for (path, props) in &pages {
        let props_json =
            serde_json::to_string(props).map_err(|e| RenderError::Codec(e.to_string()))?;
        let result = engine.render_page(&props_json)?;
        files.push(SiteFile::new(
            path.clone(),
            shell::wrap(path, props, &props_json, &result),
        ));
    }
    files.push(SiteFile::new("style.css", assets::STYLE_CSS));
    files.push(SiteFile::new("client.js", assets::CLIENT_JS));
    Ok(Site { files })
}

/// Renders the whole documentation site for `graph` under `config`, using the
/// embedded `QuickJS` engine.
///
/// # Panics
///
/// Panics if the embedded engine or SSR bundle fails — a build-asset defect,
/// deterministic and independent of user model data. Use [`try_render_site`] to
/// handle the failure instead.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn render_site(graph: &Graph, config: &DocConfig) -> Site {
    try_render_site(graph, config).expect("svelte doc SSR failed (bundle or engine defect)")
}

/// Renders the whole documentation site with the embedded `QuickJS` engine,
/// returning a [`RenderError`] on failure.
///
/// # Errors
///
/// Returns [`RenderError`] when the `QuickJS` runtime cannot be created, the
/// bundle fails to evaluate, `SSR.renderPage` throws, or props (de)serialisation
/// fails.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_render_site(graph: &Graph, config: &DocConfig) -> Result<Site, RenderError> {
    let engine = QuickJsEngine::new()?;
    try_render_site_with(graph, config, &engine)
}

/// Renders the documentation as Markdown files with the diagrams inlined as
/// self-contained SVG — a static, engine-free alternative to the Svelte site
/// ([`render_site`]). One `.md` per module plus an `index.md`, mirroring the
/// HTML site's paths. Needs no SSR engine, so it is available on every target.
#[must_use]
pub fn render_markdown_site(graph: &Graph, config: &DocConfig) -> Site {
    Site {
        files: render_md::pages_to_markdown(&render::build_pages(graph, config)),
    }
}
