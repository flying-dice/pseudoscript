//! Lexer, parser, and AST for `PseudoScript` (`LANG.md` §2, §10).
//!
//! This is the foundation crate every other `PseudoScript` crate depends on. It
//! turns source text into tokens ([`tokenize`]) and a typed syntax tree
//! ([`parse`]), emitting the shared [`Diagnostic`] type. It is WASM-safe: no
//! threads, filesystem, time, or native-only dependencies, and no I/O.
//!
//! # Surfaces
//!
//! - **Lexical:** [`tokenize`] / [`render_tokens`] produce the conformance token
//!   stream (`KIND@line:col "lexeme"`). [`lex`] additionally returns
//!   [`Trivia`] (comments, blank-line gaps) for the formatter.
//! - **Syntax:** [`parse`] yields [`Parsed`] — a [`Module`] plus diagnostics.
//!   The parser never panics and recovers from errors.
//!
//! # Example
//!
//! ```
//! use pseudoscript_syntax::parse;
//!
//! let parsed = parse("public system Banking;");
//! assert!(parsed.diagnostics.is_empty());
//! assert_eq!(parsed.ast.items.len(), 1);
//! ```

pub mod ast;
mod diagnostic;
mod lexer;
mod parser;
mod span;
mod token;

pub use ast::Module;
pub use diagnostic::{Diagnostic, Severity};
pub use lexer::{Lexed, SpannedTrivia, Trivia, lex, render_tokens, tokenize};
pub use parser::{Parsed, parse};
pub use span::{LineIndex, Span};
pub use token::{Token, TokenKind};
