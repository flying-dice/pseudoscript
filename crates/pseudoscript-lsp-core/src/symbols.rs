//! Document outline, folding ranges, and workspace symbol search.
//!
//! `document_symbols` and `folding_ranges` are pure over one module's source;
//! `workspace_symbols` searches the resolved [`Workspace`] index by name.

use lsp_types::{
    DocumentSymbol, FoldingRange, FoldingRangeKind, Location, SymbolInformation, SymbolKind, Url,
};
use pseudoscript_model::{SymbolKind as ModelKind, Workspace, ast};
use pseudoscript_syntax::{LineIndex, Span, parse};

use crate::convert::span_to_range;

/// The hierarchical outline of `src`: nodes with their callables and nested
/// declarations, `data` types with their fields or variants, and aliases.
#[must_use]
pub fn document_symbols(src: &str) -> Vec<DocumentSymbol> {
    let module = parse(src).ast;
    let index = LineIndex::new(src);
    module
        .items
        .iter()
        .map(|item| item_symbol(item, src, &index))
        .collect()
}

/// The outline entry for a top-level item.
fn item_symbol(item: &ast::Item, src: &str, index: &LineIndex) -> DocumentSymbol {
    match item {
        ast::Item::Decl(decl) => decl_symbol(decl, src, index),
        ast::Item::Feature(feature) => symbol(
            &feature.name.name,
            SymbolKind::EVENT,
            feature.span,
            feature.name.span,
            Vec::new(),
            src,
            index,
        ),
    }
}

/// The outline entry for a declaration, with its members as children.
fn decl_symbol(decl: &ast::Decl, src: &str, index: &LineIndex) -> DocumentSymbol {
    match &decl.kind {
        ast::DeclKind::Person(node)
        | ast::DeclKind::System(node)
        | ast::DeclKind::Container(node)
        | ast::DeclKind::Component(node) => {
            let children = node
                .body
                .iter()
                .flatten()
                .map(|member| match member {
                    ast::BodyMember::Callable(callable) => symbol(
                        &callable.name.name,
                        SymbolKind::METHOD,
                        callable.span,
                        callable.name.span,
                        Vec::new(),
                        src,
                        index,
                    ),
                    ast::BodyMember::Decl(inner) => decl_symbol(inner, src, index),
                })
                .collect();
            symbol(
                &node.name.name,
                SymbolKind::NAMESPACE,
                decl.span,
                node.name.span,
                children,
                src,
                index,
            )
        }
        ast::DeclKind::Data(data) => {
            let children = data_children(data, src, index);
            symbol(
                &data.name.name,
                SymbolKind::CLASS,
                decl.span,
                data.name.span,
                children,
                src,
                index,
            )
        }
        // §3.6: a constant is a value name with no children.
        ast::DeclKind::Constant(constant) => symbol(
            &constant.name.name,
            SymbolKind::CONSTANT,
            decl.span,
            constant.name.span,
            Vec::new(),
            src,
            index,
        ),
    }
}

/// The field/variant children of a `data` declaration.
fn data_children(data: &ast::Data, src: &str, index: &LineIndex) -> Vec<DocumentSymbol> {
    match &data.body {
        ast::DataBody::Record(fields) => fields
            .iter()
            .map(|f| {
                symbol(
                    &f.name.name,
                    SymbolKind::FIELD,
                    f.span,
                    f.name.span,
                    Vec::new(),
                    src,
                    index,
                )
            })
            .collect(),
        ast::DataBody::Union(variants) => variants
            .iter()
            .map(|v| {
                symbol(
                    &v.name.name,
                    SymbolKind::ENUM_MEMBER,
                    v.span,
                    v.name.span,
                    Vec::new(),
                    src,
                    index,
                )
            })
            .collect(),
        ast::DataBody::BlackBox => Vec::new(),
    }
}

/// Builds a [`DocumentSymbol`]; `full` is the whole declaration, `name_span`
/// the identifier the editor selects on reveal.
#[allow(deprecated)] // `deprecated` field is required by the struct literal.
fn symbol(
    name: &str,
    kind: SymbolKind,
    full: Span,
    name_span: Span,
    children: Vec<DocumentSymbol>,
    src: &str,
    index: &LineIndex,
) -> DocumentSymbol {
    DocumentSymbol {
        name: name.to_owned(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range: span_to_range(src, index, full),
        selection_range: span_to_range(src, index, name_span),
        children: (!children.is_empty()).then_some(children),
    }
}

/// The foldable regions of `src`: every multi-line declaration and statement
/// block. Spans come from the shared engine; this only maps them to line-based
/// `FoldingRange`s and drops single-line spans.
#[must_use]
pub fn folding_ranges(src: &str) -> Vec<FoldingRange> {
    let index = LineIndex::new(src);
    pseudoscript_model::folding_ranges(src)
        .into_iter()
        .filter_map(|range| fold(range, &index))
        .collect()
}

/// A folding range for a byte span, or `None` when it does not cross a line.
fn fold(range: pseudoscript_model::FoldRange, index: &LineIndex) -> Option<FoldingRange> {
    let (start_line, _) = index.line_col(range.start);
    let (end_line, _) = index.line_col(range.end);
    (end_line > start_line).then_some(FoldingRange {
        start_line: start_line - 1,
        start_character: None,
        end_line: end_line - 1,
        end_character: None,
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    })
}

/// Workspace symbols whose name contains `query` (case-insensitive); an empty
/// query returns every declared symbol. The server has already mapped each
/// module to a URI via `locate`.
#[must_use]
pub fn workspace_symbols(
    ws: &Workspace,
    query: &str,
    locate: impl Fn(&str) -> Option<(Url, String)>,
) -> Vec<SymbolInformation> {
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    for symbol in ws.symbols() {
        if !needle.is_empty() && !symbol.name.to_lowercase().contains(&needle) {
            continue;
        }
        let module = symbol.fqn.rsplit_once("::").map_or("", |(m, _)| m);
        let Some((uri, source)) = locate(module) else {
            continue;
        };
        let index = LineIndex::new(&source);
        out.push(symbol_information(
            symbol,
            uri,
            span_to_range(&source, &index, symbol.span),
        ));
    }
    out
}

/// Builds a flat [`SymbolInformation`] for a workspace symbol.
#[allow(deprecated)] // `deprecated` field is required by the struct literal.
fn symbol_information(
    symbol: &pseudoscript_model::Symbol,
    uri: Url,
    range: lsp_types::Range,
) -> SymbolInformation {
    SymbolInformation {
        name: symbol.name.clone(),
        kind: match symbol.kind {
            ModelKind::Data => SymbolKind::CLASS,
            _ => SymbolKind::NAMESPACE,
        },
        tags: None,
        deprecated: None,
        location: Location { uri, range },
        container_name: Some(symbol.fqn.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_nests_members_under_node() {
        let src = "//! m\n\nsystem Bank {\n  open(): void {}\n}\n\ndata Acct { id: uuid }\n";
        let symbols = document_symbols(src);
        let bank = symbols.iter().find(|s| s.name == "Bank").expect("Bank");
        assert_eq!(bank.kind, SymbolKind::NAMESPACE);
        let children = bank.children.as_ref().expect("members");
        assert_eq!(children[0].name, "open");
        assert_eq!(children[0].kind, SymbolKind::METHOD);

        let acct = symbols.iter().find(|s| s.name == "Acct").expect("Acct");
        assert_eq!(acct.kind, SymbolKind::CLASS);
        assert_eq!(acct.children.as_ref().unwrap()[0].name, "id");
    }

    #[test]
    fn folds_multi_line_node_and_block() {
        let src = "//! m\n\nsystem Bank {\n  open(): void {\n    return\n  }\n}\n";
        let ranges = folding_ranges(src);
        // the node body and the callable body each fold (end strictly past start)
        assert!(ranges.len() >= 2, "{ranges:?}");
        assert!(ranges.iter().all(|r| r.end_line > r.start_line));
        // single-line constructs do not produce a fold
        assert!(folding_ranges("//! m\n\nsystem S;\n").is_empty());
    }

    #[test]
    fn workspace_search_matches_by_substring() {
        let modules = [("banking", "//! banking\n\npublic system Ledger;\n")];
        let ws = Workspace::build(modules.iter().map(|(f, s)| ((*f).to_owned(), parse(s).ast)));
        let locate = |fqn: &str| {
            (fqn == "banking").then(|| {
                (
                    Url::parse("file:///banking.pds").unwrap(),
                    modules[0].1.to_owned(),
                )
            })
        };
        let hits = workspace_symbols(&ws, "ledg", locate);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Ledger");
    }
}
