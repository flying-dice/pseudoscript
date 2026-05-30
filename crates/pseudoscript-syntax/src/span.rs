//! Source positions: byte [`Span`]s and a [`LineIndex`] mapping offsets to
//! 1-based line/column pairs.

use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` into the source text.
///
/// Offsets are byte offsets, not character offsets. Columns derived from a
/// span via [`LineIndex`] count bytes from the line start, matching the
/// conformance token goldens (which use byte columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the first byte in the range.
    pub start: u32,
    /// Byte offset one past the last byte in the range.
    pub end: u32,
}

impl Span {
    /// Builds a span from a start and end byte offset.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Length of the span in bytes.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Whether the span covers zero bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// A span covering both `self` and `other` (and any gap between them).
    #[must_use]
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// Precomputed newline offsets for turning byte offsets into 1-based
/// line/column pairs in `O(log n)`.
///
/// Build once per source string and reuse for every lookup.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line. `line_starts[0] == 0`.
    line_starts: Vec<u32>,
    len: u32,
}

impl LineIndex {
    /// Builds the index by scanning `src` for `\n` bytes.
    #[must_use]
    pub fn new(src: &str) -> Self {
        let mut line_starts = vec![0u32];
        line_starts.extend(
            src.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| (i + 1) as u32),
        );
        Self {
            line_starts,
            len: src.len() as u32,
        }
    }

    /// Returns the 1-based `(line, column)` for `offset`.
    ///
    /// `column` counts bytes from the line start, so the first character on a
    /// line is column 1. An offset past the end clamps to the end.
    #[must_use]
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.len);
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        let col = offset - self.line_starts[line];
        ((line + 1) as u32, col + 1)
    }
}
