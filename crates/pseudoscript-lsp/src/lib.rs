//! The `PseudoScript` language server.
//!
//! Wires [`pseudoscript_model`] diagnostics and [`pseudoscript_format`]
//! formatting into a [tower-lsp](https://docs.rs/tower-lsp) server speaking LSP
//! over stdio. The CLI's `pds lsp` subcommand calls [`run_stdio`].
//!
//! Unlike the lower crates this one owns I/O (stdin/stdout, async runtime) and
//! is therefore **not** WASM-safe — that is intentional; it is the edge.
//!
//! # Surfaces
//!
//! - [`analysis`] — pure `text -> LSP value` functions (diagnostics, formatting
//!   edit, hover, definition). These carry the behaviour and are unit/BDD
//!   tested without spawning a server.
//! - [`Backend`] — the [`tower_lsp::LanguageServer`] implementation and document
//!   store.
//! - [`run_stdio`] — build the server over stdin/stdout and serve until exit.

pub mod analysis;
mod complete;
mod convert;
mod infer;
mod refs;
use pseudoscript_model::resolve;
mod semantic;
mod server;
mod symbols;
mod workspace;

pub use server::Backend;

use tower_lsp::{LspService, Server};

/// Runs the language server over stdio until the client sends `exit`.
///
/// Builds a [`Backend`] over [`tokio::io::stdin`] / [`tokio::io::stdout`] and
/// drives the tower-lsp [`Server`]. This is the function `pds lsp` awaits.
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
