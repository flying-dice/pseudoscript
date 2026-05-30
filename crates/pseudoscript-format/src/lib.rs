//! Canonical formatter for `PseudoScript` (`LANG.md` §2.1, §10).
//!
//! [`format`] parses source with [`pseudoscript_syntax::parse`] and, if there
//! are no parse-error diagnostics, pretty-prints the syntax tree (carrying its
//! comment and blank-line trivia) to one canonical form. Source that does not
//! parse is never reformatted: [`format`] returns [`FormatError::Parse`] and the
//! caller keeps the original text.
//!
//! The output is **idempotent** (`format(format(x)) == format(x)`) and
//! **semantics-preserving** (the reformatted text re-parses to a structurally
//! equivalent tree — only whitespace and comment layout change).
//!
//! It is WASM-safe: no threads, filesystem, time, or native dependencies.
//!
//! # Canonical style
//!
//! - Two-space indentation per nesting level.
//! - Declaration order (§2.1): `///` doc block → `#[..]` macros (one per line) →
//!   `public` modifier → construct keyword.
//! - Doc blocks render summary lines, then a blank `///` line then the extended
//!   lines if any, then each tag on its own `///` line (ADR-009).
//! - `name: Type`, `: ReturnType`, one space around `=`, `Result<T, E>` generics
//!   joined with `, `, array suffix `T[]` with no space (ADR-008).
//! - Black-box `;` bodies stay `;`; disclosed `{ }` bodies put each member on its
//!   own line; an empty disclosed block renders `{ }`.
//! - One statement per line; `if (c) { … } else { … }` brace style.
//! - `//` and `/* */` comments survive via leading trivia; runs of blank lines
//!   collapse to at most one.
//!
//! # Example
//!
//! ```
//! use pseudoscript_format::format;
//!
//! let out = format("public   system   Banking ;").unwrap();
//! assert_eq!(out, "public system Banking;\n");
//! ```

mod printer;

use pseudoscript_syntax::{Module, parse};

/// Why [`format`] could not produce canonical output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The source had parse errors, so it was left unformatted. The variant
    /// carries the rendered error diagnostics (one per line) for context; the
    /// caller should keep its original text.
    Parse(Vec<String>),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Parse(errors) => {
                write!(f, "cannot format source with parse errors")?;
                for e in errors {
                    write!(f, "\n  {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// Formats `src` to canonical `PseudoScript`.
///
/// Returns the formatted text on success. If `src` has any parse-error
/// diagnostics it is returned unchanged to the caller via
/// [`FormatError::Parse`]; the formatter never emits lossy output for
/// unparseable input.
///
/// # Errors
///
/// Returns [`FormatError::Parse`] when the source does not parse cleanly.
pub fn format(src: &str) -> Result<String, FormatError> {
    let parsed = parse(src);
    let errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(FormatError::Parse(errors));
    }
    Ok(format_module(&parsed.ast))
}

/// Pretty-prints an already-parsed [`Module`] to canonical text.
///
/// Primarily for the LSP, which holds a parsed tree. Prefer [`format`] for the
/// string-to-string surface. The trailing newline matches [`format`].
#[must_use]
pub fn format_module(module: &Module) -> String {
    printer::print_module(module)
}
