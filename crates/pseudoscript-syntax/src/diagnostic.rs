//! The shared diagnostic type emitted by every `PseudoScript` compiler stage.

use serde::{Deserialize, Serialize};

use crate::span::Span;

/// How serious a [`Diagnostic`] is.
///
/// `Error` means the artifact is invalid (the parse failed, a reference did not
/// resolve); `Warning` and `Info` are advisory and never block compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// The input is invalid for this stage.
    Error,
    /// Advisory; the input is still valid.
    Warning,
    /// Informational only.
    Info,
}

/// A single message about a span of source.
///
/// This is the one diagnostic type every crate in the workspace emits, so a
/// driver can collect lexer, parser, and checker output into one ordered list.
/// `code` is an optional stable identifier (e.g. `"E0001"`) for tooling; the
/// human-facing text is `message`. `code_description` is an optional URL the
/// `code` resolves to — an article explaining the rule — which an LSP edge
/// surfaces as the diagnostic's clickable link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity of the message.
    pub severity: Severity,
    /// Source range the message refers to.
    pub span: Span,
    /// Optional stable diagnostic code for tooling.
    pub code: Option<String>,
    /// Optional URL the `code` resolves to (an article explaining the rule).
    pub code_description: Option<String>,
    /// Human-readable description.
    pub message: String,
}

impl Diagnostic {
    /// Builds an `Error` diagnostic with no code.
    #[must_use]
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            span,
            code: None,
            code_description: None,
            message: message.into(),
        }
    }

    /// Builds a `Warning` diagnostic with no code.
    #[must_use]
    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            span,
            code: None,
            code_description: None,
            message: message.into(),
        }
    }

    /// Returns a copy of this diagnostic with `code` attached.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Returns a copy of this diagnostic with the `code`'s article URL attached.
    #[must_use]
    pub fn with_code_description(mut self, url: impl Into<String>) -> Self {
        self.code_description = Some(url.into());
        self
    }

    /// Whether this diagnostic is an `Error`.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}
