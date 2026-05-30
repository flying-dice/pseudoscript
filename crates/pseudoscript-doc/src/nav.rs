//! Graph navigation shared by the documentation renderers (this crate and
//! `pseudoscript-doc-svelte`): which nodes a module page lists, the nesting
//! rule, owned callables, and a node's documented children.
//!
//! Pure and deterministic — every result is sorted by FQN, no clock or
//! randomness — so both renderers project the same structure from one source.

use pseudoscript_model::{Graph, GraphNode, NodeKind};

/// Every module FQN in the graph, de-duplicated and sorted.
#[must_use]
pub fn sorted_modules(graph: &Graph) -> Vec<String> {
    let mut modules: Vec<String> = graph.nodes().iter().map(|n| n.module.clone()).collect();
    modules.sort_unstable();
    modules.dedup();
    modules
}

/// The top-level structural/data nodes a module page lists, sorted by FQN.
/// Callables are documented under their owner, not as top-level entries.
#[must_use]
pub fn module_top_level<'a>(graph: &'a Graph, module: &str) -> Vec<&'a GraphNode> {
    let mut nodes: Vec<&GraphNode> = graph
        .nodes()
        .iter()
        .filter(|n| n.module == module && n.kind != NodeKind::Callable)
        .filter(|n| !is_nested(graph, n))
        .collect();
    nodes.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    nodes
}

/// Whether `node` nests under a parent documented on the *same* module page — a
/// nested decl or a container/component whose `for` parent is in this module.
/// Such nodes still get a section (so anchors resolve), just not a sidebar
/// entry. A node whose parent lives in another module (a container `for` a
/// cross-module system, `LANG.md` §8.1) is not nested here: it lists under its
/// own module.
#[must_use]
pub fn is_nested(graph: &Graph, node: &GraphNode) -> bool {
    node.parent
        .as_deref()
        .and_then(|p| graph.node(p))
        .is_some_and(|parent| parent.module == node.module)
}

/// The callables owned by `owner_fqn`, sorted by FQN.
#[must_use]
pub fn callables_of<'a>(graph: &'a Graph, owner_fqn: &'a str) -> Vec<&'a GraphNode> {
    let mut callables: Vec<&GraphNode> = graph
        .children_of(owner_fqn)
        .filter(|n| n.kind == NodeKind::Callable)
        .collect();
    callables.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    callables
}

/// A node's same-module, non-callable children (its `for`-children or nested
/// declarations), sorted by FQN. Callables are documented inside their owner's
/// section, not as their own tree entries.
#[must_use]
pub fn child_nodes<'a>(graph: &'a Graph, node: &GraphNode) -> Vec<&'a GraphNode> {
    let mut children: Vec<&GraphNode> = graph
        .nodes()
        .iter()
        .filter(|n| {
            n.kind != NodeKind::Callable
                && n.module == node.module
                && n.parent.as_deref() == Some(node.fqn.as_str())
        })
        .collect();
    children.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    children
}
