//! HTML text escaping.
//!
//! All user-supplied text — node names, `///` documentation, tags, the site
//! title — flows through [`escape`] before it reaches the page, so a model can
//! never inject markup into the generated site.

use std::borrow::Cow;

/// Escapes the five HTML-significant characters in `text`.
///
/// Returns a borrowed [`Cow`] when `text` needs no escaping, so the common case
/// (most identifiers and prose) allocates nothing.
///
/// # Examples
///
/// ```
/// # use pseudoscript_doc::escape;
/// assert_eq!(escape("plain"), "plain");
/// assert_eq!(escape("a < b & \"c\""), "a &lt; b &amp; &quot;c&quot;");
/// ```
#[must_use]
pub fn escape(text: &str) -> Cow<'_, str> {
    if !text.bytes().any(is_significant) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    Cow::Owned(out)
}

/// Whether `byte` is one of the characters [`escape`] rewrites.
fn is_significant(byte: u8) -> bool {
    matches!(byte, b'&' | b'<' | b'>' | b'"' | b'\'')
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn passes_plain_text_through_borrowed() {
        assert!(matches!(
            escape("no markup here"),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn escapes_every_significant_character() {
        assert_eq!(
            escape("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn escapes_fqn_separators_untouched() {
        assert_eq!(escape("banking::core::Ledger"), "banking::core::Ledger");
    }
}
