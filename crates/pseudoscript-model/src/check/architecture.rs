//! Architectural-principle lints over the resolved graph (`LANG.md` §9).
//!
//! Beyond §8.2 visibility — which a `public` component still satisfies — these
//! advise on C4 structure. Each violation is a `Warning` (the model stays valid)
//! carrying a stable `PDS-ARCH-NNN` code and the URL of the article that explains
//! the principle, which an LSP edge surfaces as the diagnostic's clickable link.
//!
//! - **PDS-ARCH-001 — facade bypass (backdooring):** a cross-module body call
//!   that lands on an internal `component` instead of the container's published
//!   face. The defining rule, already guarded for the repo's own models by
//!   `crates/pseudoscript/tests/model_conformance.rs`; this surfaces it live.
//! - **PDS-ARCH-002 — cyclic dependency:** the module dependency graph (derived
//!   from `Call` edges) contains a cycle.
//! - **PDS-ARCH-003 — system-boundary bypass:** a cross-`system` call whose
//!   target is a `container` of the other system — couple to the system's
//!   published face, not its containers.

use std::collections::{BTreeMap, BTreeSet};

use pseudoscript_syntax::Diagnostic;

use crate::graph::{Edge, EdgeKind, Graph, GraphNode, NodeKind};

/// Where the `docs/principles/` articles are published. Each rule's `code`
/// resolves to `{ARTICLE_BASE}{slug}`; the one place the host/path changes.
const ARTICLE_BASE: &str =
    "https://github.com/flying-dice/pseudoscript/blob/main/docs/principles/";

/// One architectural-principle lint rule: its stable code and the filename of the
/// article (under [`ARTICLE_BASE`]) that documents it.
struct ArchRule {
    code: &'static str,
    slug: &'static str,
}

const BACKDOOR: ArchRule = ArchRule {
    code: "PDS-ARCH-001",
    slug: "PDS-ARCH-001-backdooring-facade.md",
};
const CYCLE: ArchRule = ArchRule {
    code: "PDS-ARCH-002",
    slug: "PDS-ARCH-002-cyclic-dependency.md",
};
const SYSTEM_BOUNDARY: ArchRule = ArchRule {
    code: "PDS-ARCH-003",
    slug: "PDS-ARCH-003-system-boundary.md",
};

/// One lint finding tagged with the module its span lies in, so the per-file
/// diagnostics path ([`check_for_module`]) can attribute it to the right file.
struct Finding {
    module: String,
    diag: Diagnostic,
}

/// Every architectural warning across the whole workspace graph.
pub(crate) fn check(graph: &Graph) -> Vec<Diagnostic> {
    findings(graph).into_iter().map(|f| f.diag).collect()
}

/// The architectural warnings whose offending call originates in `module` — the
/// per-file slice the IDE attributes to that module's source.
pub(crate) fn check_for_module(graph: &Graph, module: &str) -> Vec<Diagnostic> {
    findings(graph)
        .into_iter()
        .filter(|f| f.module == module)
        .map(|f| f.diag)
        .collect()
}

fn findings(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    check_backdoor(graph, &mut out);
    check_cycles(graph, &mut out);
    check_system_boundary(graph, &mut out);
    out
}

/// Builds a `Warning` stamped with a rule's code and article URL.
fn warn(rule: &ArchRule, diag: Diagnostic) -> Diagnostic {
    diag.with_code(rule.code)
        .with_code_description(format!("{ARTICLE_BASE}{}", rule.slug))
}

/// Iterates the resolved `Call` edges — those whose endpoints both name a graph
/// node — yielding each with its source and target node. The shared front of
/// every edge-based detector.
fn call_edges(graph: &Graph) -> impl Iterator<Item = (&Edge, &GraphNode, &GraphNode)> {
    graph
        .edges()
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Call)
        .filter_map(move |edge| {
            Some((edge, graph.node(&edge.from)?, graph.node(&edge.to)?))
        })
}

/// PDS-ARCH-001 — a `Call` edge into a `component` declared in a different module
/// than its caller backdoors the container's published face.
fn check_backdoor(graph: &Graph, out: &mut Vec<Finding>) {
    for (edge, from, to) in call_edges(graph) {
        if to.kind == NodeKind::Component && from.module != to.module {
            let message = format!(
                "cross-module call reaches into internal component `{}` — call its container's published face instead (facade/gateway)",
                to.fqn
            );
            out.push(Finding {
                module: from.module.clone(),
                diag: warn(&BACKDOOR, Diagnostic::warning(edge.span, message)),
            });
        }
    }
}

/// PDS-ARCH-003 — a `Call` edge into a `container` of another system reaches past
/// that system's boundary. The component-target case is PDS-ARCH-001, so a
/// `container` target is the only one judged here.
fn check_system_boundary(graph: &Graph, out: &mut Vec<Finding>) {
    for (edge, from, to) in call_edges(graph) {
        if to.kind != NodeKind::Container {
            continue;
        }
        let (Some(from_sys), Some(to_sys)) =
            (system_of(graph, &from.fqn), system_of(graph, &to.fqn))
        else {
            continue;
        };
        if from_sys != to_sys {
            let message = format!(
                "call crosses a system boundary into container `{}` of another system — couple to the system's published face, not its containers",
                to.fqn
            );
            out.push(Finding {
                module: from.module.clone(),
                diag: warn(&SYSTEM_BOUNDARY, Diagnostic::warning(edge.span, message)),
            });
        }
    }
}

/// PDS-ARCH-002 — the module dependency graph (each cross-module `Call` edge is a
/// `source.module -> target.module` arc) must stay acyclic. Emits one warning per
/// strongly-connected component of more than one module, at a representative call
/// edge into the cycle.
fn check_cycles(graph: &Graph, out: &mut Vec<Finding>) {
    // Module adjacency and a representative call edge per crossed pair. BTree keys
    // keep the SCC search and output deterministic.
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut rep: BTreeMap<(&str, &str), (String, pseudoscript_syntax::Span)> = BTreeMap::new();
    for (edge, from, to) in call_edges(graph) {
        if from.module == to.module {
            continue;
        }
        adj.entry(&from.module).or_default().insert(&to.module);
        rep.entry((&from.module, &to.module))
            .or_insert_with(|| (from.module.clone(), edge.span));
    }

    for scc in strongly_connected(&adj) {
        if scc.len() < 2 {
            continue;
        }
        // A representative arc inside the cycle: a crossed pair (a, b) both in the
        // SCC. Deterministic via the sorted pair scan.
        let Some(((_, _), (module, span))) = rep
            .iter()
            .find(|((a, b), _)| scc.contains(*a) && scc.contains(*b))
            .map(|(k, v)| (*k, v.clone()))
        else {
            continue;
        };
        let members = scc.iter().copied().collect::<Vec<_>>().join(", ");
        let message = format!(
            "modules form a dependency cycle ({members}) — break it by extracting a shared contract or inverting one dependency"
        );
        out.push(Finding {
            module,
            diag: warn(&CYCLE, Diagnostic::warning(span, message)),
        });
    }
}

/// The enclosing `system` FQN of a node — walk `parent` up the C4 nesting
/// (component -> container -> system). `None` when the node sits under no system.
///
/// A malformed model can wire `parent` into a cycle (`container A for B`,
/// `container B for A` — a §4 diagnostic, but the graph still builds the pointers);
/// the `seen` guard bounds the walk so the lint never hangs on invalid input.
fn system_of<'a>(graph: &'a Graph, fqn: &str) -> Option<&'a str> {
    let mut node = graph.node(fqn)?;
    let mut seen = BTreeSet::new();
    while seen.insert(node.fqn.as_str()) {
        if node.kind == NodeKind::System {
            return Some(&node.fqn);
        }
        node = graph.node(node.parent.as_deref()?)?;
    }
    None
}

/// Tarjan's strongly-connected components over the module adjacency. Returns each
/// component as a sorted module list, in a deterministic order.
fn strongly_connected<'a>(adj: &BTreeMap<&'a str, BTreeSet<&'a str>>) -> Vec<BTreeSet<&'a str>> {
    // Every node that appears as a source or a target.
    let mut nodes: BTreeSet<&str> = adj.keys().copied().collect();
    for targets in adj.values() {
        nodes.extend(targets.iter().copied());
    }

    struct State<'a> {
        index: BTreeMap<&'a str, u32>,
        low: BTreeMap<&'a str, u32>,
        on_stack: BTreeSet<&'a str>,
        stack: Vec<&'a str>,
        next: u32,
        out: Vec<BTreeSet<&'a str>>,
    }

    fn visit<'a>(
        st: &mut State<'a>,
        adj: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        v: &'a str,
    ) {
        st.index.insert(v, st.next);
        st.low.insert(v, st.next);
        st.next += 1;
        st.stack.push(v);
        st.on_stack.insert(v);
        if let Some(targets) = adj.get(v) {
            for &w in targets {
                if !st.index.contains_key(w) {
                    visit(st, adj, w);
                    let low = st.low[v].min(st.low[w]);
                    st.low.insert(v, low);
                } else if st.on_stack.contains(w) {
                    let low = st.low[v].min(st.index[w]);
                    st.low.insert(v, low);
                }
            }
        }
        if st.low[v] == st.index[v] {
            let mut component = BTreeSet::new();
            while let Some(w) = st.stack.pop() {
                st.on_stack.remove(w);
                component.insert(w);
                if w == v {
                    break;
                }
            }
            st.out.push(component);
        }
    }

    let mut st = State {
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        on_stack: BTreeSet::new(),
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for &v in &nodes {
        if !st.index.contains_key(v) {
            visit(&mut st, adj, v);
        }
    }
    st.out
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::graph::Graph;
    use crate::model::Workspace;
    use pseudoscript_syntax::{Diagnostic, Severity, parse};

    /// The architectural diagnostics for a workspace of `(fqn, source)` modules.
    fn diags(modules: &[(&str, &str)]) -> Vec<Diagnostic> {
        let ws = Workspace::build(
            modules
                .iter()
                .map(|(fqn, src)| ((*fqn).to_owned(), parse(src).ast)),
        );
        check(&Graph::build(&ws))
    }

    fn codes(modules: &[(&str, &str)]) -> Vec<String> {
        diags(modules).into_iter().filter_map(|d| d.code).collect()
    }

    #[test]
    fn backdoor_cross_module_component_call_warns() {
        let ds = diags(&[
            (
                "a",
                "//! a\n\npublic system Sys;\npublic container Box for a::Sys;\npublic component Comp for a::Box {\n  run(): void;\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Other;\npublic container Caller for b::Other {\n  go(): void { a::Comp.run() }\n}\n",
            ),
        ]);
        let hit = ds
            .iter()
            .find(|d| d.code.as_deref() == Some("PDS-ARCH-001"))
            .expect("PDS-ARCH-001 fires on a cross-module component call");
        assert_eq!(hit.severity, Severity::Warning);
        assert!(
            hit.code_description
                .as_deref()
                .is_some_and(|url| url.contains("PDS-ARCH-001")),
            "the warning carries its article URL: {:?}",
            hit.code_description
        );
        // Reaching a component is PDS-ARCH-001, never the container rule.
        assert!(!ds.iter().any(|d| d.code.as_deref() == Some("PDS-ARCH-003")));
    }

    #[test]
    fn same_module_component_call_is_clean() {
        let ds = diags(&[(
            "shop",
            "//! shop\n\npublic system App;\npublic container Box for shop::App;\npublic component Repo for shop::Box {\n  run(): void;\n}\npublic container Caller for shop::App {\n  go(): void { shop::Repo.run() }\n}\n",
        )]);
        assert!(ds.is_empty(), "{:?}", codes(&[]));
    }

    #[test]
    fn module_dependency_cycle_warns() {
        // Two containers in the *same* system (so the system-boundary rule stays
        // silent) whose modules call each other — a module cycle.
        let cs = codes(&[
            ("s", "//! s\n\npublic system Sys;\n"),
            (
                "a",
                "//! a\n\npublic container Ca for s::Sys {\n  go(): void { b::Cb.run() }\n  run(): void;\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic container Cb for s::Sys {\n  run(): void { a::Ca.run() }\n}\n",
            ),
        ]);
        assert!(cs.iter().any(|c| c == "PDS-ARCH-002"), "{cs:?}");
        assert!(!cs.iter().any(|c| c == "PDS-ARCH-003"), "{cs:?}");
    }

    #[test]
    fn cross_system_container_call_warns() {
        // A container of system A calls a container of system B — a boundary bypass,
        // not a cycle, not a component reach-in.
        let cs = codes(&[
            (
                "a",
                "//! a\n\npublic system Sa;\npublic container Ca for a::Sa {\n  go(): void { b::Cb.run() }\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic system Sb;\npublic container Cb for b::Sb {\n  run(): void;\n}\n",
            ),
        ]);
        assert!(cs.iter().any(|c| c == "PDS-ARCH-003"), "{cs:?}");
        assert!(!cs.iter().any(|c| c == "PDS-ARCH-001"), "{cs:?}");
        assert!(!cs.iter().any(|c| c == "PDS-ARCH-002"), "{cs:?}");
    }

    #[test]
    fn malformed_parent_cycle_does_not_hang() {
        // `container Ca for b::Cb` and `container Cb for a::Ca` wire `parent` into a
        // cycle (a §4 violation the graph still builds). The lint must terminate.
        let cs = codes(&[
            (
                "a",
                "//! a\n\npublic container Ca for b::Cb {\n  go(): void { b::Cb.run() }\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic container Cb for a::Ca {\n  run(): void {}\n}\n",
            ),
        ]);
        // Reaching here means `system_of` did not loop; neither node has a system
        // ancestor, so the boundary rule stays silent.
        assert!(!cs.iter().any(|c| c == "PDS-ARCH-003"), "{cs:?}");
    }

    #[test]
    fn same_system_cross_module_container_call_is_clean() {
        // Container-to-container across modules but within one system is fine.
        let cs = codes(&[
            ("s", "//! s\n\npublic system Sys;\n"),
            (
                "a",
                "//! a\n\npublic container Ca for s::Sys {\n  go(): void { b::Cb.run() }\n}\n",
            ),
            (
                "b",
                "//! b\n\npublic container Cb for s::Sys {\n  run(): void;\n}\n",
            ),
        ]);
        assert!(cs.is_empty(), "{cs:?}");
    }
}
