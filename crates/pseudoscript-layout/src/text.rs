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
}
