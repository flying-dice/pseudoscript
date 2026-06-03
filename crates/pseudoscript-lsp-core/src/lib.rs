//! The transport-neutral `PseudoScript` LSP API.
//!
//! Pure `text + position -> lsp_types value` functions — diagnostics, hover,
//! definition, references, completion, semantic tokens, folding, symbols,
//! formatting. No transport, no async runtime, no `tower-lsp`: this crate is
//! WASM-safe, so both edges share one implementation —
//!
//! - [`pseudoscript-lsp`] wraps it in a tower-lsp stdio server, and
//! - [`pseudoscript-ide`] serialises the same `lsp_types` results to JSON.
//!
//! Types are the standalone [`lsp_types`] crate at the version `tower-lsp`
//! re-exports, so the server passes them through with no conversion.

pub mod analysis;
pub mod complete;
pub mod convert;
pub mod refs;
pub mod semantic;
pub mod symbols;

pub use pseudoscript_model::resolve;
