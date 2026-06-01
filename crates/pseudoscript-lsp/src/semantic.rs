//! LSP adapter over the shared semantic-token engine.
//!
//! The AST-aware colouring lives in `pseudoscript_model::semantic`; this module
//! advertises the legend and delta-encodes the engine's byte-offset tokens into
//! the `lsp_types` wire format (single-line, non-overlapping, delta-encoded).

use pseudoscript_model::{SemKind, semantic_tokens as model_semantic_tokens};
use pseudoscript_syntax::LineIndex;
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use crate::convert::offset_to_position;

/// The `declaration` modifier bit (bit 0 of the legend's modifier list).
const MOD_DECLARATION: u32 = 1 << 0;

/// The token types this server emits, in legend order. The index MUST match
/// [`sem_index`].
fn token_types() -> Vec<SemanticTokenType> {
    vec![
        SemanticTokenType::NAMESPACE,
        SemanticTokenType::TYPE,
        SemanticTokenType::CLASS,
        SemanticTokenType::PARAMETER,
        SemanticTokenType::VARIABLE,
        SemanticTokenType::PROPERTY,
        SemanticTokenType::ENUM_MEMBER,
        SemanticTokenType::METHOD,
        SemanticTokenType::KEYWORD,
        SemanticTokenType::COMMENT,
        SemanticTokenType::STRING,
        SemanticTokenType::NUMBER,
        SemanticTokenType::DECORATOR,
    ]
}

/// The legend the server advertises and the encoder indexes into.
#[must_use]
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: token_types(),
        token_modifiers: vec![SemanticTokenModifier::DECLARATION],
    }
}

/// The legend index for a model token kind (must match [`token_types`] order).
fn sem_index(kind: SemKind) -> u32 {
    match kind {
        SemKind::Namespace => 0,
        SemKind::Type => 1,
        SemKind::Class => 2,
        SemKind::Parameter => 3,
        SemKind::Variable => 4,
        SemKind::Property => 5,
        SemKind::EnumMember => 6,
        SemKind::Method => 7,
        SemKind::Keyword => 8,
        SemKind::Comment => 9,
        SemKind::String => 10,
        SemKind::Number => 11,
        SemKind::Decorator => 12,
    }
}

/// Computes the semantic tokens for `src` as a full-document, delta-encoded set.
///
/// The engine already returns sorted, non-overlapping byte-offset tokens; this
/// drops any spanning more than one line (the protocol requires single-line
/// tokens) and delta-encodes the rest.
#[must_use]
pub fn semantic_tokens(src: &str) -> SemanticTokens {
    let index = LineIndex::new(src);
    let mut data = Vec::new();
    let mut prev_line = 0;
    let mut prev_start = 0;

    for token in model_semantic_tokens(src) {
        let start = offset_to_position(src, &index, token.start);
        let end = offset_to_position(src, &index, token.end);
        if end.line != start.line || end.character <= start.character {
            continue;
        }
        let delta_line = start.line - prev_line;
        let delta_start = if delta_line == 0 {
            start.character - prev_start
        } else {
            start.character
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: end.character - start.character,
            token_type: sem_index(token.kind),
            token_modifiers_bitset: if token.declaration {
                MOD_DECLARATION
            } else {
                0
            },
        });
        prev_line = start.line;
        prev_start = start.character;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A decoded token in absolute coordinates, for assertion convenience.
    #[derive(Debug, PartialEq, Eq)]
    struct Decoded {
        line: u32,
        start: u32,
        len: u32,
        ty: SemanticTokenType,
        declared: bool,
    }

    fn decode(src: &str) -> Vec<Decoded> {
        let types = token_types();
        let tokens = semantic_tokens(src);
        let mut line = 0;
        let mut start = 0;
        tokens
            .data
            .iter()
            .map(|t| {
                if t.delta_line == 0 {
                    start += t.delta_start;
                } else {
                    line += t.delta_line;
                    start = t.delta_start;
                }
                Decoded {
                    line,
                    start,
                    len: t.length,
                    ty: types[t.token_type as usize].clone(),
                    declared: t.token_modifiers_bitset & MOD_DECLARATION != 0,
                }
            })
            .collect()
    }

    fn at<'a>(decoded: &'a [Decoded], src: &str, needle: &str) -> &'a Decoded {
        let offset = src.find(needle).expect("substring present") as u32;
        let line = src[..offset as usize].matches('\n').count() as u32;
        let line_start = src[..offset as usize].rfind('\n').map_or(0, |nl| nl + 1) as u32;
        let start = offset - line_start;
        decoded
            .iter()
            .find(|d| d.line == line && d.start == start)
            .unwrap_or_else(|| panic!("no token at {needle:?}"))
    }

    #[test]
    fn node_name_is_a_declared_namespace() {
        let src = "//! m\n\npublic system Banking;\n";
        let decoded = decode(src);
        let token = at(&decoded, src, "Banking");
        assert_eq!(token.ty, SemanticTokenType::NAMESPACE);
        assert!(token.declared);
        assert_eq!(token.len, 7);
    }

    #[test]
    fn data_name_and_members() {
        let src = "//! m\n\ndata Account { id: uuid }\n";
        let decoded = decode(src);
        assert_eq!(at(&decoded, src, "data").ty, SemanticTokenType::KEYWORD);
        assert_eq!(at(&decoded, src, "Account").ty, SemanticTokenType::CLASS);
        assert_eq!(at(&decoded, src, "id").ty, SemanticTokenType::PROPERTY);
        assert_eq!(at(&decoded, src, "uuid").ty, SemanticTokenType::TYPE);
    }
}
