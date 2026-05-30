//! Static documentation-site generation for `PseudoScript` (`LANG.md` §9.3,
//! ADR-017) — the core of `pds doc`.
//!
//! This crate sits above [`pseudoscript_model`] and [`pseudoscript_emit`]: it
//! turns a resolved [`Graph`](pseudoscript_model::Graph) into a cargo-doc-style
//! static site — HTML pages carrying each node's `///` documentation, tags,
//! visibility, and relationships, with the C4 and sequence diagrams embedded as
//! inline SVG.
//!
//! It performs **no I/O**: [`render_site`] returns the site as in-memory
//! [`SiteFile`]s and the CLI writes them. There are no threads, no clock, and no
//! randomness, so output is deterministic and the crate is unit-testable.
//!
//! # Surfaces
//!
//! - [`render_site`] — the entry point: a resolved graph + a [`DocConfig`] → a
//!   [`Site`].
//! - [`DocConfig`] / [`Theme`] — site presentation, filled by the CLI from
//!   `[doc]` in `pds.toml`.
//! - [`Site`] / [`SiteFile`] — the generated files (`index.html`, `style.css`,
//!   `app.js`, and one `module/<fqn>.html` per module).
//! - [`escape`] — the HTML text escaper, exposed for the CLI and tests.
//!
//! # URL scheme
//!
//! `index.html` is the entry point. Shared assets sit at the site root
//! (`style.css`, `app.js`). Each module is a page at `module/<dotted-fqn>.html`,
//! and each node is the `#<slug>` anchor of its section within that page. A
//! configured `logo` is embedded by its filename at the site root, so the CLI
//! copies the source file there.
//!
//! # Example
//!
//! ```
//! use pseudoscript_doc::{DocConfig, render_site};
//! use pseudoscript_model::{WorkspaceModule, graph};
//!
//! let g = graph(&[WorkspaceModule::new(
//!     "shop",
//!     "//! shop\npublic person Customer;\npublic system Shop;",
//! )]);
//! let site = render_site(&g, &DocConfig::default());
//! assert!(site.file("index.html").is_some());
//! ```

mod assets;
mod config;
mod escape;
mod render;
mod site;
mod url;

pub use config::{DocConfig, Theme};
pub use escape::escape;
pub use render::render_site;
pub use site::{Site, SiteFile};
