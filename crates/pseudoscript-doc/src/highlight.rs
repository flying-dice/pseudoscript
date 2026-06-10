//! Build-time syntax highlighting for fenced code blocks.
//!
//! `pds`/`pseudoscript` blocks highlight through the toolchain's own lexer —
//! exact, zero extra dependencies, and the language that matters most here.
//! Output is class-based `<span class="tok-…">` runs coloured by the site
//! stylesheet, so one deterministic render serves every theme. Any other
//! language passes through escaped and unhighlighted (an embedded grammar set
//! for common languages is a future extension).

use pseudoscript_syntax::{Token, TokenKind, Trivia, lex};

use crate::escape::escape;

/// Highlights `source` as `lang` into class-based HTML (the inner content of a
/// `<pre><code>` block). Unknown languages return the escaped source verbatim.
pub(crate) fn highlight(lang: &str, source: &str) -> String {
    if matches!(lang, "pds" | "pseudoscript") {
        pds_tokens(source)
    } else {
        escape(source).into_owned()
    }
}

/// One classed source run: its byte span and token class (`None` = plain).
struct Run {
    start: usize,
    end: usize,
    class: Option<&'static str>,
}

/// Lexes the snippet and wraps each token (and comment) in its kind-classed
/// span, slicing the **raw source** by span so markers and whitespace survive
/// exactly (doc-token lexemes are stripped; their spans are not).
fn pds_tokens(source: &str) -> String {
    let lexed = lex(source);
    let mut runs: Vec<Run> = lexed
        .tokens
        .iter()
        .map(|token| Run {
            start: token.span.start as usize,
            end: token.span.end as usize,
            class: token_class(token),
        })
        .collect();
    runs.extend(lexed.trivia.iter().filter_map(|t| {
        matches!(t.trivia, Trivia::LineComment(_) | Trivia::BlockComment(_)).then_some(Run {
            start: t.span.start as usize,
            end: t.span.end as usize,
            class: Some("tok-comment"),
        })
    }));
    runs.sort_by_key(|r| r.start);

    let mut out = String::with_capacity(source.len() * 2);
    let mut cursor = 0;
    for run in runs {
        let (start, end) = (run.start.min(source.len()), run.end.min(source.len()));
        if start < cursor || end <= start {
            continue; // overlapping or empty span — keep the earlier run
        }
        out.push_str(&escape(&source[cursor..start]));
        let text = escape(&source[start..end]);
        match run.class {
            Some(class) => {
                out.push_str("<span class=\"");
                out.push_str(class);
                out.push_str("\">");
                out.push_str(&text);
                out.push_str("</span>");
            }
            None => out.push_str(&text),
        }
        cursor = end;
    }
    out.push_str(&escape(&source[cursor.min(source.len())..]));
    out
}

/// The CSS class for a token kind; `None` renders plain.
fn token_class(token: &Token) -> Option<&'static str> {
    match token.kind {
        TokenKind::KwSystem
        | TokenKind::KwContainer
        | TokenKind::KwComponent
        | TokenKind::KwPerson
        | TokenKind::KwData
        | TokenKind::KwConstant
        | TokenKind::KwFor
        | TokenKind::KwFrom
        | TokenKind::KwPublic
        | TokenKind::KwSelf
        | TokenKind::KwReturn
        | TokenKind::KwOk
        | TokenKind::KwErr
        | TokenKind::KwSome
        | TokenKind::KwNone
        | TokenKind::KwIf
        | TokenKind::KwElse
        | TokenKind::KwWhile
        | TokenKind::KwIn
        | TokenKind::KwTrue
        | TokenKind::KwFalse
        | TokenKind::KwFeature
        | TokenKind::KwGiven
        | TokenKind::KwWhen
        | TokenKind::KwThen
        | TokenKind::KwAnd
        | TokenKind::KwBut => Some("tok-kw"),
        TokenKind::String => Some("tok-string"),
        TokenKind::Number => Some("tok-number"),
        TokenKind::Doc | TokenKind::InnerDoc => Some("tok-doc"),
        TokenKind::Tag | TokenKind::HashLBracket => Some("tok-tag"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pds_blocks_highlight_keywords_strings_and_docs() {
        let html = highlight("pds", "/// A shop.\npublic system Shop;\n");
        assert!(html.contains("<span class=\"tok-kw\">public</span>"), "{html}");
        assert!(html.contains("<span class=\"tok-kw\">system</span>"), "{html}");
        assert!(html.contains("tok-doc"), "{html}");
        assert!(html.contains("Shop"), "plain idents survive: {html}");
    }

    #[test]
    fn unknown_languages_escape_verbatim() {
        let html = highlight("rust", "let x = a < b;");
        assert_eq!(html, "let x = a &lt; b;");
        assert!(!html.contains("<span"));
    }

    #[test]
    fn raw_source_survives_byte_for_byte_modulo_escaping() {
        // The classed runs reassemble the snippet exactly — markers,
        // whitespace, comments — so copy-to-clipboard gets the real source.
        let src = "// note\nsystem S; // trailing\n";
        let html = highlight("pds", src);
        let stripped = html
            .replace("<span class=\"tok-kw\">", "")
            .replace("<span class=\"tok-comment\">", "")
            .replace("</span>", "");
        assert_eq!(stripped, escape(src));
    }
}
