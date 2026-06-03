//! Shared text measurement. A headless layout crate cannot query a font engine,
//! so widths are a deterministic per-character approximation tuned to the
//! web-ide monospace font; this keeps Rust-computed positions identical to what
//! the browser draws. Widths are integer tenths-of-a-pixel to stay exact and
//! platform-independent (no float rounding across the wasm boundary).

use serde::{Deserialize, Serialize};

/// Per-character widths (in tenths of a pixel) plus the horizontal padding a
/// pill/label reserves around its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextMetrics {
    /// Tenths-of-a-pixel per character for message / edge labels.
    pub label_char_tenths: i32,
    /// Tenths-of-a-pixel per character for heavier node / participant titles.
    pub title_char_tenths: i32,
    /// Tenths-of-a-pixel per character for the dimmed description / parent line.
    pub desc_char_tenths: i32,
    /// Padding added around a measured label (both sides combined).
    pub label_pad: i32,
}

impl Default for TextMetrics {
    fn default() -> Self {
        // The web-ide labels in 12.5px mono and titles in ~13px mono; a mono
        // glyph advances ~0.62em, so ~7.8px/char. Sized to fit (slightly
        // generous) so a message signature never overflows the lane between two
        // lifelines.
        Self {
            label_char_tenths: 78,
            title_char_tenths: 80,
            // The dimmed description renders ~11.5px mono (~6px/char), matching
            // the C4 card's summary so wrapping agrees across both renderers.
            desc_char_tenths: 60,
            label_pad: 14,
        }
    }
}

/// `ceil(count * tenths / 10)`, with `count` saturating into `i32`.
fn scale(count: usize, tenths: i32) -> i32 {
    let n = i32::try_from(count).unwrap_or(i32::MAX);
    (n.saturating_mul(tenths) + 9) / 10
}

impl TextMetrics {
    /// The pill width a label occupies: text width plus padding. Empty text
    /// occupies nothing.
    #[must_use]
    pub fn label_width(&self, text: &str) -> i32 {
        let count = text.chars().count();
        if count == 0 {
            0
        } else {
            scale(count, self.label_char_tenths) + self.label_pad
        }
    }

    /// The bare width a title's text occupies (no padding).
    #[must_use]
    pub fn title_width(&self, text: &str) -> i32 {
        scale(text.chars().count(), self.title_char_tenths)
    }

    /// Greedily word-wraps a description into at most `max_lines` lines that each
    /// fit `width_px` at the description font, appending an ellipsis to the last
    /// line on overflow. Pure and deterministic, so a card's height (line count)
    /// and its drawn text agree. Mirrors the C4 card's `wrap_summary`.
    #[must_use]
    pub fn wrap_desc(&self, text: &str, width_px: i32, max_lines: usize) -> Vec<String> {
        // Char budget from the available width; at least one char so a degenerate
        // width still makes progress. `width_px * 10 / tenths` = px / px-per-char.
        let max_chars = usize::try_from(
            (width_px.max(0).saturating_mul(10) / self.desc_char_tenths.max(1)).max(1),
        )
        .unwrap_or(1);

        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let fits = current.is_empty()
                || current.chars().count() + 1 + word.chars().count() <= max_chars;
            if fits {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }

        let overflow = lines.len() > max_lines;
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut().filter(|_| overflow) {
            let budget = max_chars.saturating_sub(1).max(1);
            if last.chars().count() > budget {
                let kept: String = last.chars().take(budget).collect();
                *last = kept;
                last.push('\u{2026}');
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_label_is_zero() {
        assert_eq!(TextMetrics::default().label_width(""), 0);
    }

    #[test]
    fn label_width_scales_and_pads() {
        let m = TextMetrics::default();
        // 4 chars * 7.8 = 31.2 -> ceil 32, + 14 pad = 46.
        assert_eq!(m.label_width("Find"), 46);
    }

    #[test]
    fn title_width_has_no_pad() {
        // 5 chars * 8.0 = 40.
        assert_eq!(TextMetrics::default().title_width("Check"), 40);
    }

    #[test]
    fn wrap_desc_short_text_is_one_line_no_ellipsis() {
        let lines = TextMetrics::default().wrap_desc("hello world", 400, 2);
        assert_eq!(lines, vec!["hello world".to_owned()]);
    }

    #[test]
    fn wrap_desc_caps_lines_and_ellipsises_overflow() {
        let m = TextMetrics::default();
        let lines = m.wrap_desc("one two three four five six seven eight nine ten", 60, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines.last().is_some_and(|l| l.ends_with('\u{2026}')));
    }
}
