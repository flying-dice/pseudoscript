//! The SSR engine boundary and its native `QuickJS` implementation.
//!
//! [`SsrEngine`] is the JSON-string-in / JSON-string-out seam between the page
//! model and the JavaScript renderer. The native build supplies
//! [`QuickJsEngine`], an embedded `QuickJS` ([`rquickjs`]) that runs the bundled
//! Svelte SSR code in-process. A wasm host can instead implement [`SsrEngine`]
//! against the host's own JavaScript engine, loading the same `ssr.js` bundle —
//! so `rquickjs` (which compiles `QuickJS` C) never enters a wasm build. The
//! engine is target-gated in `Cargo.toml` to match.

use crate::props::RenderedPage;

/// A failure inside the SSR renderer. Every variant indicates a defect in the
/// bundle, the engine, or the props codec — not user model data; a well-formed
/// props value always renders.
#[derive(Debug)]
pub enum RenderError {
    /// Engine creation or bundle evaluation failed.
    Engine(String),
    /// `SSR.renderPage` was missing, threw, or returned a non-string.
    Call(String),
    /// Serialising props or deserialising the result failed.
    Codec(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Engine(msg) => write!(f, "SSR engine: {msg}"),
            RenderError::Call(msg) => write!(f, "SSR.renderPage: {msg}"),
            RenderError::Codec(msg) => write!(f, "SSR props/result codec: {msg}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// The seam between [`crate::try_render_site_with`] and the JavaScript SSR
/// renderer: given the serialised page props, return the rendered
/// `{head, body}`. One implementation per host — embedded `QuickJS` natively, a
/// host-JS bridge under wasm.
pub trait SsrEngine {
    /// Renders one page from its serialised props JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when the underlying engine call fails or its
    /// result cannot be decoded into `{head, body}`.
    fn render_page(&self, props_json: &str) -> Result<RenderedPage, RenderError>;
}

#[cfg(not(target_arch = "wasm32"))]
pub use quickjs::QuickJsEngine;

/// The native engine: an embedded `QuickJS` runtime. Gated out on wasm, where
/// the host already provides a JavaScript engine.
#[cfg(not(target_arch = "wasm32"))]
mod quickjs {
    use rquickjs::{CatchResultExt, Context, Function, Object, Runtime};

    use super::{RenderError, SsrEngine};
    use crate::assets::SSR_JS;
    use crate::props::RenderedPage;

    /// A `console` that swallows output, so the Svelte server runtime's
    /// `console.*` calls neither throw (`QuickJS` defines no `console`) nor
    /// pollute stdout. Evaluated before the bundle.
    const CONSOLE_SHIM: &str = "globalThis.console={log(){},info(){},warn(){},\
        error(){},debug(){},trace(){},assert(){}};";

    /// An embedded `QuickJS` engine. Owns a [`Runtime`] + [`Context`] for its
    /// lifetime, evaluating the SSR bundle once so `globalThis.SSR.renderPage`
    /// stays resident; each page is then a cheap call. `QuickJS` is
    /// single-threaded and [`Runtime`]/[`Context`] are not [`Send`], matching
    /// the single-threaded `render_site`.
    pub struct QuickJsEngine {
        // `Context` holds a strong reference to the `Runtime`; both are kept
        // alive so the evaluated bundle stays resident.
        _runtime: Runtime,
        context: Context,
    }

    impl QuickJsEngine {
        /// Creates the runtime, installs the console shim, and evaluates the SSR
        /// bundle so `globalThis.SSR.renderPage` is defined.
        ///
        /// # Errors
        ///
        /// Returns [`RenderError::Engine`] if the runtime/context cannot be
        /// created or the bundle fails to evaluate.
        pub fn new() -> Result<Self, RenderError> {
            let runtime = Runtime::new().map_err(|e| RenderError::Engine(e.to_string()))?;
            let context =
                Context::full(&runtime).map_err(|e| RenderError::Engine(e.to_string()))?;
            context.with(|ctx| -> Result<(), RenderError> {
                ctx.eval::<(), _>(CONSOLE_SHIM)
                    .catch(&ctx)
                    .map_err(|e| RenderError::Engine(e.to_string()))?;
                ctx.eval::<(), _>(SSR_JS)
                    .catch(&ctx)
                    .map_err(|e| RenderError::Engine(e.to_string()))?;
                Ok(())
            })?;
            Ok(Self {
                _runtime: runtime,
                context,
            })
        }
    }

    impl SsrEngine for QuickJsEngine {
        fn render_page(&self, props_json: &str) -> Result<RenderedPage, RenderError> {
            let result_json = self.context.with(|ctx| -> Result<String, RenderError> {
                let ssr: Object = ctx
                    .globals()
                    .get("SSR")
                    .catch(&ctx)
                    .map_err(|e| RenderError::Call(e.to_string()))?;
                let render_page: Function = ssr
                    .get("renderPage")
                    .catch(&ctx)
                    .map_err(|e| RenderError::Call(e.to_string()))?;
                render_page
                    .call((props_json,))
                    .catch(&ctx)
                    .map_err(|e| RenderError::Call(e.to_string()))
            })?;
            serde_json::from_str(&result_json).map_err(|e| RenderError::Codec(e.to_string()))
        }
    }
}
