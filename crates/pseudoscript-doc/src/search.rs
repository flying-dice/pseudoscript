//! The static full-text search index the client palette queries.
//!
//! Shipped as `search-index.js`, a classic-script assignment of an entry array
//! to `window.__PDS_SEARCH__` — never fetched, so it loads under `file://`
//! (Chrome blocks `fetch` and module scripts from disk). One record per
//! linkable node, module page, and authored doc page, plus the universe and
//! health pages. Hrefs are root-relative; the client prepends the page's known
//! prefix. Sorted by href then FQN, serialised with struct field order —
//! deterministic.

use pseudoscript_model::{Graph, NodeKind};
use pulldown_cmark::{Event, Parser};
use serde::Serialize;

use crate::config::DocConfig;
use crate::site::SiteFile;
use crate::url::{UrlMap, doc_page_path, module_page_path};

/// One search record.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchEntry {
    /// The symbol FQN (or the page path for non-symbol entries).
    fqn: String,
    /// The display name.
    name: String,
    /// The kind word (a node kind, `module`, `doc`, or `page`).
    kind: String,
    /// The one-line summary.
    summary: String,
    /// The plain-text body, capped — enough for term matching, not storage.
    text: String,
    /// The declaring module (empty for pages).
    module: String,
    /// The root-relative href.
    href: String,
}

/// How much body text a record carries.
const TEXT_CAP: usize = 300;

/// Builds `search-index.js` for the whole site.
pub(crate) fn build_search_index(graph: &Graph, config: &DocConfig, urls: &UrlMap) -> SiteFile {
    let mut entries: Vec<SearchEntry> = Vec::new();

    for node in graph.nodes() {
        if node.kind == NodeKind::Callable {
            continue; // documented inside its owner's section
        }
        let Some(url) = urls.get(&node.fqn) else {
            continue;
        };
        entries.push(SearchEntry {
            fqn: node.fqn.clone(),
            name: node.name.clone(),
            kind: node.kind.keyword().to_owned(),
            summary: node.doc.summary.clone().unwrap_or_default(),
            text: cap(node.doc.extended.as_deref().unwrap_or_default()),
            module: node.module.clone(),
            href: format!("{}#{}", url.page, url.anchor),
        });
    }

    let mut modules: Vec<&str> = graph.nodes().iter().map(|n| n.module.as_str()).collect();
    modules.sort_unstable();
    modules.dedup();
    for module in modules {
        entries.push(SearchEntry {
            fqn: module.to_owned(),
            name: module.to_owned(),
            kind: "module".to_owned(),
            summary: String::new(),
            text: String::new(),
            module: module.to_owned(),
            href: module_page_path(module),
        });
    }

    for group in &config.docs {
        for page in &group.pages {
            entries.push(SearchEntry {
                fqn: doc_page_path(&page.path),
                name: page.title.clone(),
                kind: "doc".to_owned(),
                summary: group.title.clone(),
                text: cap(&markdown_text(&page.markdown)),
                module: String::new(),
                href: doc_page_path(&page.path),
            });
        }
    }

    for (name, href, summary) in [
        ("3D Universe", "universe.html", "The model in 3D — structure and flows"),
        ("Architecture Health", "health.html", "Errors, warnings, and principle lints"),
    ] {
        entries.push(SearchEntry {
            fqn: href.to_owned(),
            name: name.to_owned(),
            kind: "page".to_owned(),
            summary: summary.to_owned(),
            text: String::new(),
            module: String::new(),
            href: href.to_owned(),
        });
    }

    entries.sort_by(|a, b| a.href.cmp(&b.href).then_with(|| a.fqn.cmp(&b.fqn)));
    let json = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_owned());
    SiteFile::new(
        "search-index.js",
        format!("window.__PDS_SEARCH__={json};"),
    )
}

/// Strips Markdown to plain text: the text and code events space-joined, with
/// runs of whitespace collapsed so inline markup leaves no double spaces.
fn markdown_text(markdown: &str) -> String {
    let mut out = String::new();
    for event in Parser::new(markdown) {
        if let Event::Text(t) | Event::Code(t) = event {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&t);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Caps body text at a char boundary near [`TEXT_CAP`].
fn cap(text: &str) -> String {
    if text.len() <= TEXT_CAP {
        return text.to_owned();
    }
    let mut end = TEXT_CAP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_model::{WorkspaceModule, graph};

    #[test]
    fn index_is_a_window_global_with_sorted_entries() {
        let g = graph(&[WorkspaceModule::new(
            "m",
            "//! m\n/// The shop.\npublic system Shop;\n",
        )]);
        let file = build_search_index(&g, &DocConfig::default(), &UrlMap::build(&g));
        assert_eq!(file.path, "search-index.js");
        assert!(file.contents.starts_with("window.__PDS_SEARCH__=["));
        assert!(file.contents.contains("m::Shop"));
        assert!(file.contents.contains("universe.html"));
        assert!(file.contents.contains("health.html"));
    }

    #[test]
    fn markdown_strips_to_plain_text() {
        assert_eq!(markdown_text("# Hi\n\nSome **bold** `code`."), "Hi Some bold code .");
    }
}
