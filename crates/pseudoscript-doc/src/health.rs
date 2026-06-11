//! Architecture health: positions the host's diagnostics in the documentation.
//!
//! Attribution is positional: within a finding's module, the node whose
//! name-span starts closest **before** the diagnostic's span owns it; a
//! callable lifts to its owner (callables have no section of their own); a span
//! no node precedes falls back to the module page with no anchor. The health
//! page lists every finding sorted by severity (errors first), module, then
//! line; affected sections carry the same findings as inline badges.

use pseudoscript_model::{Graph, GraphNode, NodeKind};

use crate::diag::DiagnosticInput;
use crate::props::{HealthEntry, HealthProps, SectionDiagnostic};
use crate::url::{UrlMap, module_page_path};

/// The health page body: every finding attributed, sorted, and counted.
pub(crate) fn build_health(
    graph: &Graph,
    diagnostics: &[DiagnosticInput],
    urls: &UrlMap,
    prefix: &str,
) -> HealthProps {
    let mut entries: Vec<HealthEntry> = diagnostics
        .iter()
        .map(|d| {
            let node = owning_node(graph, d);
            let href = node.and_then(|n| urls.get(&n.fqn)).map_or_else(
                || format!("{prefix}{}", module_page_path(&d.module)),
                |url| format!("{prefix}{}#{}", url.page, url.anchor),
            );
            HealthEntry {
                module: d.module.clone(),
                severity: d.severity.clone(),
                code: d.code.clone(),
                code_url: d.code_url.clone(),
                message: d.message.clone(),
                line: d.line,
                column: d.column,
                node_fqn: node.map(|n| n.fqn.clone()).unwrap_or_default(),
                href,
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.module.cmp(&b.module))
            .then_with(|| a.line.cmp(&b.line))
    });
    let error_count = entries.iter().filter(|e| e.severity == "error").count();
    let warning_count = entries.iter().filter(|e| e.severity == "warning").count();
    HealthProps {
        entries,
        error_count,
        warning_count,
    }
}

/// The findings attributed to `node`, as its inline section badges.
pub(crate) fn badges_for(
    graph: &Graph,
    node: &GraphNode,
    diagnostics: &[DiagnosticInput],
) -> Vec<SectionDiagnostic> {
    diagnostics
        .iter()
        .filter(|d| owning_node(graph, d).is_some_and(|owner| owner.fqn == node.fqn))
        .map(|d| SectionDiagnostic {
            severity: d.severity.clone(),
            code: d.code.clone(),
            code_url: d.code_url.clone(),
            message: d.message.clone(),
            line: d.line,
        })
        .collect()
}

/// Errors before warnings before anything else.
fn severity_rank(severity: &str) -> u8 {
    match severity {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

/// The node whose **section** a finding belongs on: the same-module node whose
/// name span starts closest before (or at) the diagnostic's span, a callable
/// lifted to its owner. `None` when no node precedes the span.
fn owning_node<'g>(graph: &'g Graph, d: &DiagnosticInput) -> Option<&'g GraphNode> {
    let owner = graph
        .nodes()
        .iter()
        .filter(|n| n.module == d.module && span_start(n) <= d.start)
        .max_by_key(|n| span_start(n))?;
    if owner.kind == NodeKind::Callable {
        let parent = owner.parent.as_deref()?;
        return graph.node(parent);
    }
    Some(owner)
}

/// A node's name-span start, as the byte offset attribution compares.
fn span_start(node: &GraphNode) -> u32 {
    node.span.start
}
