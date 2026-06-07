//! LSP adapter over the shared completion engine.
//!
//! The context-aware logic lives in `pseudoscript_model::complete`; this module
//! only maps a `Position` to a byte offset and the engine's neutral
//! [`CompletionKind`] to `lsp_types::CompletionItemKind`.

use lsp_types::{CompletionItem, CompletionItemKind, Position};
use pseudoscript_model::{CompletionKind, Workspace, completion as model_completion};

use crate::convert::position_to_offset;

/// Computes completion items for `position` in module `from_fqn`'s `src`.
#[must_use]
#[tracing::instrument(level = "debug", skip(ws, src))]
pub fn completion(
    ws: &Workspace,
    from_fqn: &str,
    src: &str,
    position: Position,
) -> Vec<CompletionItem> {
    let offset = position_to_offset(src, position);
    model_completion(ws, from_fqn, src, offset)
        .into_iter()
        .map(|c| CompletionItem {
            label: c.label,
            kind: Some(item_kind(c.kind)),
            detail: Some(c.detail),
            ..CompletionItem::default()
        })
        .collect()
}

/// Maps the engine's neutral kind to the LSP completion-item kind.
fn item_kind(kind: CompletionKind) -> CompletionItemKind {
    match kind {
        CompletionKind::Method => CompletionItemKind::METHOD,
        CompletionKind::Field => CompletionItemKind::FIELD,
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::Macro => CompletionItemKind::FUNCTION,
        CompletionKind::Type => CompletionItemKind::STRUCT,
        CompletionKind::Class => CompletionItemKind::CLASS,
        CompletionKind::Module => CompletionItemKind::MODULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::offset_to_position;
    use pseudoscript_syntax::{LineIndex, parse};

    fn workspace(modules: &[(&str, &str)]) -> Workspace {
        Workspace::build(
            modules
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
        )
    }

    /// Completion labels at the byte `offset` in module `from`, through the
    /// LSP adapter (`Position` in, `lsp_types` out).
    fn labels_at(ws: &Workspace, from: &str, src: &str, offset: u32) -> Vec<String> {
        let pos = offset_to_position(src, &LineIndex::new(src), offset);
        completion(ws, from, src, pos)
            .into_iter()
            .map(|c| c.label)
            .collect()
    }

    #[test]
    fn adapter_scopes_members_with_prefix() {
        let src =
            "//! m\n\nsystem S {\n  run() {\n    self.he\n  }\n  helper(x: number): uuid;\n}\n";
        let ws = workspace(&[("m", src)]);
        let offset = (src.find("self.he").unwrap() + "self.he".len()) as u32;
        let labels = labels_at(&ws, "m", src, offset);
        assert!(labels.contains(&"helper".to_owned()), "{labels:?}");
        assert!(
            !labels.contains(&"system".to_owned()),
            "general scope leaked: {labels:?}"
        );
    }

    #[test]
    fn adapter_maps_kinds() {
        let src = "//! m\n\n";
        let ws = workspace(&[("m", src)]);
        let pos = offset_to_position(src, &LineIndex::new(src), src.len() as u32);
        let items = completion(&ws, "m", src, pos);
        let kw = items
            .iter()
            .find(|c| c.label == "system")
            .expect("keyword present");
        assert_eq!(kw.kind, Some(CompletionItemKind::KEYWORD));
    }
}
