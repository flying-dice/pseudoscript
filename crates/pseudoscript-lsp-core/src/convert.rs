//! Byte-offset ↔ LSP position conversion.
//!
//! `PseudoScript` spans are byte offsets into the source ([`pseudoscript_syntax::Span`]),
//! and [`pseudoscript_syntax::LineIndex`] maps a byte offset to a 1-based
//! line/byte-column pair. LSP positions, by contrast, are 0-based line and
//! 0-based UTF-16 code-unit character. This module bridges the two.

use lsp_types::{Position, Range};
use pseudoscript_syntax::{LineIndex, Span};

/// Converts a byte `offset` into the source to a 0-based LSP [`Position`].
///
/// The LSP `character` field counts UTF-16 code units from the line start, so
/// this scans the bytes between the line start and `offset`, summing each
/// character's UTF-16 width. For ASCII this is exactly `byte_col - 1`.
#[must_use]
pub fn offset_to_position(src: &str, index: &LineIndex, offset: u32) -> Position {
    let (line, _byte_col) = index.line_col(offset);
    let line0 = line - 1;
    let offset = (offset as usize).min(src.len());
    let line_start = src[..offset].rfind('\n').map_or(0, |nl| nl + 1);
    let character: u32 = src[line_start..offset]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();
    Position::new(line0, character)
}

/// Converts a byte [`Span`] into an LSP [`Range`].
#[must_use]
pub fn span_to_range(src: &str, index: &LineIndex, span: Span) -> Range {
    Range::new(
        offset_to_position(src, index, span.start),
        offset_to_position(src, index, span.end),
    )
}

/// The LSP [`Range`] covering the entire source, from `(0, 0)` to the end.
#[must_use]
pub fn full_range(src: &str, index: &LineIndex) -> Range {
    Range::new(
        Position::new(0, 0),
        offset_to_position(src, index, src.len() as u32),
    )
}

/// Converts a 0-based LSP [`Position`] back to a byte offset into `src`.
///
/// Inverse of [`offset_to_position`]: walks to the requested line, then sums
/// byte widths until the UTF-16 character count reaches `position.character`.
/// A position past the end of a line clamps to that line's end.
#[must_use]
pub fn position_to_offset(src: &str, position: Position) -> u32 {
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in src.bytes().enumerate() {
        if line == position.line {
            break;
        }
        if b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    if line != position.line {
        return src.len() as u32;
    }
    let line_text = &src[line_start..];
    let mut utf16 = 0u32;
    for (byte_off, ch) in line_text.char_indices() {
        if utf16 >= position.character || ch == '\n' {
            return (line_start + byte_off) as u32;
        }
        utf16 += ch.len_utf16() as u32;
    }
    src.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_offset_round_trips() {
        let src = "public system Banking;\n";
        let index = LineIndex::new(src);
        // byte offset of "system" start is 7
        let pos = offset_to_position(src, &index, 7);
        assert_eq!(pos, Position::new(0, 7));
        assert_eq!(position_to_offset(src, pos), 7);
    }

    #[test]
    fn second_line_offset() {
        let src = "a\nbc;\n";
        let index = LineIndex::new(src);
        let pos = offset_to_position(src, &index, 4); // ';'
        assert_eq!(pos, Position::new(1, 2));
    }

    #[test]
    fn non_ascii_counts_utf16_units() {
        // "é" is one UTF-16 unit but two UTF-8 bytes; "𝄞" is two UTF-16 units.
        let src = "//é𝄞x\n";
        let index = LineIndex::new(src);
        // byte offset of 'x': 2 (//) + 2 (é) + 4 (𝄞) = 8
        let pos = offset_to_position(src, &index, 8);
        // UTF-16 units before 'x': '/'(1) '/'(1) 'é'(1) '𝄞'(2) = 5
        assert_eq!(pos, Position::new(0, 5));
    }

    #[test]
    fn full_range_spans_whole_doc() {
        let src = "ab\ncd\n";
        let index = LineIndex::new(src);
        let range = full_range(src, &index);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(2, 0));
    }
}
