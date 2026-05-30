//! The FQN → site-URL map and the site URL scheme.
//!
//! Every node lives in a section on its module's page. A module's page is
//! `module/<module-fqn>.html` (the `::` separators replaced by `.` so the path
//! is a single flat filename), and a node is the `#<slug>` anchor within it.
//! Cross-links resolve an FQN through [`UrlMap::href_to`]; only FQNs that name a
//! real node produce a link, so a reference to an unresolved or synthesised
//! endpoint renders as plain text (`LANG.md` §9.3).

use std::collections::HashMap;

use pseudoscript_model::Graph;

/// A resolved cross-reference target: the page it lives on and its anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeUrl {
    /// The module page path, e.g. `module/banking.core.html`.
    pub page: String,
    /// The in-page anchor id, e.g. `banking-core-Ledger`.
    pub anchor: String,
}

impl NodeUrl {
    /// An `href` to this target from a module page. All module pages are
    /// siblings in `module/`, so the page component is just the bare filename;
    /// cross-links never originate from the site root in the rendered output.
    #[must_use]
    pub fn href_from_module_page(&self) -> String {
        let file = self.page.rsplit('/').next().unwrap_or(&self.page);
        format!("{file}#{}", self.anchor)
    }
}

/// Maps every node FQN in a graph to its [`NodeUrl`].
#[derive(Debug, Default)]
pub struct UrlMap {
    entries: HashMap<String, NodeUrl>,
}

impl UrlMap {
    /// Builds the map from every node in `graph`.
    #[must_use]
    pub fn build(graph: &Graph) -> Self {
        let entries = graph
            .nodes()
            .iter()
            .map(|node| (node.fqn.clone(), node_url(&node.module, &node.fqn)))
            .collect();
        Self { entries }
    }

    /// The resolved URL for `fqn`, or `None` when no node carries that FQN.
    #[must_use]
    pub fn get(&self, fqn: &str) -> Option<&NodeUrl> {
        self.entries.get(fqn)
    }

    /// The `href` to `fqn` from a module page, or `None` when `fqn` resolves to
    /// no node.
    #[must_use]
    pub fn href_to(&self, fqn: &str) -> Option<String> {
        self.get(fqn).map(NodeUrl::href_from_module_page)
    }
}

/// The page path for a module FQN: `module/<dotted-fqn>.html`.
#[must_use]
pub fn module_page_path(module_fqn: &str) -> String {
    format!("module/{}.html", flatten(module_fqn))
}

/// The [`NodeUrl`] a node FQN in `module_fqn` resolves to.
fn node_url(module_fqn: &str, node_fqn: &str) -> NodeUrl {
    NodeUrl {
        page: module_page_path(module_fqn),
        anchor: anchor(node_fqn),
    }
}

/// The in-page anchor id for a node FQN: `::` and other non-id characters become
/// `-`, so the id is a valid, link-safe HTML id.
#[must_use]
pub fn anchor(node_fqn: &str) -> String {
    node_fqn
        .replace("::", "-")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Flattens an FQN's `::` separators to `.` for use in a single path segment.
fn flatten(fqn: &str) -> String {
    fqn.replace("::", ".")
}

#[cfg(test)]
mod tests {
    use super::{UrlMap, anchor, module_page_path};
    use pseudoscript_model::{WorkspaceModule, graph};

    #[test]
    fn module_path_flattens_separators() {
        assert_eq!(
            module_page_path("banking::core"),
            "module/banking.core.html"
        );
    }

    #[test]
    fn anchor_is_link_safe() {
        assert_eq!(anchor("banking::core::Ledger"), "banking-core-Ledger");
    }

    #[test]
    fn maps_known_fqn_resolves_relative_to_module_page() {
        let g = graph(&[WorkspaceModule::new(
            "banking::core",
            "//! banking::core\npublic system Bank;",
        )]);
        let map = UrlMap::build(&g);
        assert_eq!(
            map.href_to("banking::core::Bank").as_deref(),
            Some("banking.core.html#banking-core-Bank"),
        );
        assert_eq!(
            map.get("banking::core::Bank").map(|u| u.page.as_str()),
            Some("module/banking.core.html"),
        );
    }

    #[test]
    fn unknown_fqn_has_no_href() {
        let g = graph(&[WorkspaceModule::new("m", "//! m\npublic system A;")]);
        let map = UrlMap::build(&g);
        assert_eq!(map.href_to("m::Missing"), None);
    }
}
