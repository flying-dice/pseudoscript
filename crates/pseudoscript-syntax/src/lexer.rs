//! Hand-written lexer for `LANG.md` §2.
//!
//! Two surfaces are produced from one pass:
//! - the conformance token stream ([`tokenize`] / [`render_tokens`]), which
//!   discards `//` and `/* */` comments and emits `DOC`/`INNER_DOC`/`TAG`;
//! - full-fidelity [`Trivia`] (comments and blank-line gaps) for the formatter,
//!   available via [`lex`].

use crate::span::{LineIndex, Span};
use crate::token::{Token, TokenKind};

/// Non-token source between tokens, preserved for the formatter.
///
/// Doc comments, tags, macros, and modifiers are first-class tokens/AST data,
/// not trivia. Only discarded comments and blank-line gaps live here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trivia {
    /// A `//` line comment; `text` includes the leading `//`.
    LineComment(String),
    /// A `/* ... */` block comment; `text` includes the delimiters.
    BlockComment(String),
    /// One or more fully blank lines; `count` is how many newlines they span.
    BlankLines(u32),
}

/// A trivia element paired with the source span it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedTrivia {
    /// The trivia content.
    pub trivia: Trivia,
    /// Source range it occupies.
    pub span: Span,
}

/// The full result of lexing: the conformance token stream plus interleaved
/// trivia for the formatter.
#[derive(Debug, Clone)]
pub struct Lexed {
    /// Tokens in source order (no comment trivia).
    pub tokens: Vec<Token>,
    /// Comments and blank-line gaps in source order.
    pub trivia: Vec<SpannedTrivia>,
}

/// Lexes `src` into the conformance token stream (`LANG.md` §2).
///
/// `//` and `/* */` comments emit no token; `///` becomes `DOC` (+ `TAG`s) and
/// `//!` becomes `INNER_DOC`. Use [`lex`] if you also need comment trivia.
#[must_use]
pub fn tokenize(src: &str) -> Vec<Token> {
    lex(src).tokens
}

/// Lexes `src`, returning both the token stream and full-fidelity trivia.
#[must_use]
pub fn lex(src: &str) -> Lexed {
    Lexer::new(src).run()
}

/// Renders `src`'s token stream as one `KIND@line:col "lexeme"` line per token,
/// matching the conformance `.tokens` goldens.
#[must_use]
pub fn render_tokens(src: &str) -> String {
    let index = LineIndex::new(src);
    let mut out = String::new();
    for token in tokenize(src) {
        let (line, col) = index.line_col(token.span.start);
        out.push_str(token.kind.name());
        out.push('@');
        out.push_str(&line.to_string());
        out.push(':');
        out.push_str(&col.to_string());
        out.push_str(" \"");
        // The lexeme is quoted; escape interior `\` and `"` so a STRING token
        // (whose lexeme includes its own quotes) renders unambiguously, e.g.
        // `STRING@6:8 "\"GET /a#b\""`.
        for ch in token.text.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                _ => out.push(ch),
            }
        }
        out.push_str("\"\n");
    }
    out
}

struct Lexer<'src> {
    src: &'src str,
    bytes: &'src [u8],
    pos: usize,
    tokens: Vec<Token>,
    trivia: Vec<SpannedTrivia>,
}

impl<'src> Lexer<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
            trivia: Vec::new(),
        }
    }

    fn run(mut self) -> Lexed {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            match b {
                b' ' | b'\t' | b'\r' => self.pos += 1,
                b'\n' => self.consume_blank_lines(),
                b'/' => self.lex_slash(),
                b'#' => self.lex_hash(),
                b'"' => self.lex_string(),
                b'0'..=b'9' => self.lex_number(),
                _ if is_ident_start(b) => self.lex_ident(),
                _ => self.lex_punct(),
            }
        }
        Lexed {
            tokens: self.tokens,
            trivia: self.trivia,
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start as u32, end as u32),
            text: self.src[start..end].to_owned(),
        });
    }

    /// Consumes a run of newlines (and intervening horizontal whitespace),
    /// recording it as blank-line trivia when it spans more than one line.
    fn consume_blank_lines(&mut self) {
        let start = self.pos;
        let mut newlines = 0u32;
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'\n' => {
                    newlines += 1;
                    self.pos += 1;
                }
                b' ' | b'\t' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
        // A single trailing newline after a token is not a blank-line gap; only
        // record a gap when at least one fully empty line is spanned.
        if newlines >= 2 {
            self.trivia.push(SpannedTrivia {
                trivia: Trivia::BlankLines(newlines - 1),
                span: Span::new(start as u32, self.pos as u32),
            });
        }
    }

    fn lex_slash(&mut self) {
        let next = self.bytes.get(self.pos + 1).copied();
        match next {
            Some(b'/') => self.lex_line_or_doc(),
            Some(b'*') => self.lex_block_comment(),
            _ => {
                let start = self.pos;
                self.pos += 1;
                // A lone `/` is not in the grammar; surface it as punctuation
                // for the parser to reject rather than failing here.
                self.push(TokenKind::Question, start, self.pos);
            }
        }
    }

    /// Handles `//` line comment, `//!` inner doc, and `///` doc lines.
    fn lex_line_or_doc(&mut self) {
        let marker_start = self.pos;
        let third = self.bytes.get(self.pos + 2).copied();
        match third {
            Some(b'!') => {
                self.pos += 3;
                let (text_start, text_end) = self.line_body();
                let text = self.src[text_start..text_end].trim().to_owned();
                self.tokens.push(Token {
                    kind: TokenKind::InnerDoc,
                    span: Span::new(marker_start as u32, text_end as u32),
                    text,
                });
            }
            Some(b'/') => {
                self.pos += 3;
                self.lex_doc_line(marker_start);
            }
            _ => {
                // Plain `//` line comment — discarded, kept as trivia.
                self.pos += 2;
                let (_, text_end) = self.line_body();
                self.trivia.push(SpannedTrivia {
                    trivia: Trivia::LineComment(self.src[marker_start..text_end].to_owned()),
                    span: Span::new(marker_start as u32, text_end as u32),
                });
            }
        }
    }

    /// Splits a `///` line into `DOC` prose segments and `TAG` tokens.
    ///
    /// `marker_start` is the offset of the leading `/`; the first `DOC` (if any)
    /// reports that column. A `#name` run becomes a `TAG`; prose before/between
    /// tags becomes trimmed `DOC` segments. A blank `///` line emits one empty
    /// `DOC` so the parser can detect the summary/body split (ADR-009).
    fn lex_doc_line(&mut self, marker_start: usize) {
        let (body_start, body_end) = self.line_body();
        let body = &self.src[body_start..body_end];

        let mut segment_start = body_start;
        let mut i = body_start;
        let mut emitted = false;
        // The first DOC segment reports the marker column; later segments use
        // their own offset.
        let mut doc_anchor = marker_start;

        while i < body_end {
            if self.bytes[i] == b'#' && is_tag(self.bytes, i, body_end) {
                self.flush_doc_segment(segment_start, i, doc_anchor, &mut emitted);
                doc_anchor = usize::MAX; // subsequent DOCs anchor at their offset
                let tag_start = i;
                i += 1;
                while i < body_end && is_ident_continue(self.bytes[i]) {
                    i += 1;
                }
                self.tokens.push(Token {
                    kind: TokenKind::Tag,
                    span: Span::new(tag_start as u32, i as u32),
                    text: self.src[tag_start..i].to_owned(),
                });
                emitted = true;
                segment_start = i;
            } else {
                i += 1;
            }
        }
        self.flush_doc_segment(segment_start, body_end, doc_anchor, &mut emitted);

        // Empty `///` line: emit a blank DOC so doc-block splitting works.
        if !emitted {
            self.tokens.push(Token {
                kind: TokenKind::Doc,
                span: Span::new(marker_start as u32, body_end as u32),
                text: String::new(),
            });
        }
        let _ = body;
    }

    /// Emits a trimmed `DOC` token for `[seg_start, seg_end)` if it is
    /// non-empty after trimming. `anchor` is the span start to report (the
    /// marker column for the first segment, `usize::MAX` to use `seg_start`).
    fn flush_doc_segment(
        &mut self,
        seg_start: usize,
        seg_end: usize,
        anchor: usize,
        emitted: &mut bool,
    ) {
        let text = self.src[seg_start..seg_end].trim();
        if text.is_empty() {
            return;
        }
        let span_start = if anchor == usize::MAX {
            seg_start
        } else {
            anchor
        };
        self.tokens.push(Token {
            kind: TokenKind::Doc,
            span: Span::new(span_start as u32, seg_end as u32),
            text: text.to_owned(),
        });
        *emitted = true;
    }

    /// Returns `[start, end)` of the current line's body (from `self.pos` up to
    /// but not including the newline), advancing `self.pos` to the newline.
    fn line_body(&mut self) -> (usize, usize) {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
            self.pos += 1;
        }
        (start, self.pos)
    }

    fn lex_block_comment(&mut self) {
        let start = self.pos;
        self.pos += 2;
        while self.pos < self.bytes.len() {
            if self.bytes[self.pos] == b'*' && self.bytes.get(self.pos + 1) == Some(&b'/') {
                self.pos += 2;
                break;
            }
            self.pos += 1;
        }
        self.trivia.push(SpannedTrivia {
            trivia: Trivia::BlockComment(self.src[start..self.pos].to_owned()),
            span: Span::new(start as u32, self.pos as u32),
        });
    }

    fn lex_hash(&mut self) {
        let start = self.pos;
        if self.bytes.get(self.pos + 1) == Some(&b'[') {
            self.pos += 2;
            self.push(TokenKind::HashLBracket, start, self.pos);
        } else {
            // A `#` outside a `///` line and not starting `#[` is literal prose
            // (§2.4); it has no grammar role. Skip it so it never blocks parsing.
            self.pos += 1;
        }
    }

    fn lex_string(&mut self) {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'\\' => self.pos += 2,
                b'"' => {
                    self.pos += 1;
                    break;
                }
                b'\n' => break,
                _ => self.pos += 1,
            }
        }
        self.pos = self.pos.min(self.bytes.len());
        self.push(TokenKind::String, start, self.pos);
    }

    fn lex_number(&mut self) {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        // Optional decimal part (ADR-013), only when a digit follows the dot.
        if self.bytes.get(self.pos) == Some(&b'.')
            && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit)
        {
            self.pos += 1;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        self.push(TokenKind::Number, start, self.pos);
    }

    fn lex_ident(&mut self) {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() && is_ident_continue(self.bytes[self.pos]) {
            self.pos += 1;
        }
        let text = &self.src[start..self.pos];
        let kind = TokenKind::keyword(text).unwrap_or(TokenKind::Ident);
        self.push(kind, start, self.pos);
    }

    fn lex_punct(&mut self) {
        let start = self.pos;
        let b = self.bytes[self.pos];
        let kind = match b {
            b':' => {
                if self.bytes.get(self.pos + 1) == Some(&b':') {
                    self.pos += 2;
                    self.push(TokenKind::ColonColon, start, self.pos);
                    return;
                }
                TokenKind::Colon
            }
            b'.' => TokenKind::Dot,
            b';' => TokenKind::Semi,
            b',' => TokenKind::Comma,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b'=' => TokenKind::Eq,
            b'|' => TokenKind::Pipe,
            b'?' => TokenKind::Question,
            b'<' => TokenKind::LAngle,
            b'>' => TokenKind::RAngle,
            b'!' => TokenKind::Bang,
            _ => {
                // Unknown byte: skip it (and any trailing bytes of a UTF-8 char)
                // so the lexer never stalls. The parser reports the gap.
                self.pos += 1;
                while self.pos < self.bytes.len() && (self.bytes[self.pos] & 0xC0) == 0x80 {
                    self.pos += 1;
                }
                return;
            }
        };
        self.pos += 1;
        self.push(kind, start, self.pos);
    }
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

/// Whether a `#` at `i` begins a `#name` tag: a `#` immediately followed by an
/// identifier-start byte (within the doc-line body `[.., end)`).
fn is_tag(bytes: &[u8], i: usize, end: usize) -> bool {
    i + 1 < end && is_ident_start(bytes[i + 1])
}
