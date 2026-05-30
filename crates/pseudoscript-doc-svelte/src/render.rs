//! Projects a resolved [`Graph`] into per-page [`PageProps`].
//!
//! Pure and deterministic, mirroring `pseudoscript_doc`'s render ordering:
//! modules and nodes are sorted by FQN, callables document under their owner,
//! and same-module children nest. No clock, no randomness, no I/O. The node
//! selection/nesting helpers are ports of the equivalents in
//! `pseudoscript_doc::render` so both renderers agree on structure.

use pseudoscript_doc::DocConfig;
use pseudoscript_doc::nav::{callables_of, child_nodes, module_top_level, sorted_modules};
use pseudoscript_doc::url::{UrlMap, anchor, module_page_path};
use pseudoscript_emit::{Scene, View, project};
use pseudoscript_model::{Edge, EdgeKind, Graph, GraphNode, NodeKind, Visibility};

use crate::props::{
    Diagram, IndexProps, ModuleCard, ModuleProps, NodeSection, PageBody, PageProps, RelGroup,
    RelItem, ScenarioCard, SidebarModule, SidebarNode, SiteInfo, Step,
};

/// Builds every page's props in deterministic file order: `index.html` first,
/// then one module page per module sorted by FQN. The returned path is the
/// site-relative output path (without the `module/` directory's HTML shell).
pub(crate) fn build_pages(graph: &Graph, config: &DocConfig) -> Vec<(String, PageProps)> {
    let urls = UrlMap::build(graph);
    let modules = sorted_modules(graph);

    let mut pages = Vec::with_capacity(modules.len() + 1);
    pages.push((
        "index.html".to_owned(),
        build_index(graph, config, &modules, &urls),
    ));
    for module in &modules {
        pages.push((
            module_page_path(module),
            build_module(graph, config, module, &modules, &urls),
        ));
    }
    pages
}

/// The site info for a page at the given root `prefix`.
fn site_info(config: &DocConfig, prefix: &str) -> SiteInfo {
    SiteInfo {
        name: config.name.clone(),
        theme: config.theme.attr().to_owned(),
        logo_filename: config.logo_filename().map(ToOwned::to_owned),
        prefix: prefix.to_owned(),
    }
}

// ---- index ----------------------------------------------------------------

fn build_index(graph: &Graph, config: &DocConfig, modules: &[String], urls: &UrlMap) -> PageProps {
    // The index sits at the site root: no `../` prefix on its links.
    let sidebar = build_sidebar(graph, modules, urls, "");
    let cards = modules
        .iter()
        .map(|module| {
            let count = module_top_level(graph, module).len();
            ModuleCard {
                name: module.clone(),
                href: module_page_path(module),
                meta: format!("{count} item{}", if count == 1 { "" } else { "s" }),
            }
        })
        .collect();
    let page = PageBody::Index(IndexProps {
        title: config.name.clone(),
        context_diagram: build_diagram(graph, View::Context, "Context", "System context"),
        cards,
    });
    PageProps {
        site: site_info(config, ""),
        sidebar,
        page,
    }
}

// ---- module page -----------------------------------------------------------

fn build_module(
    graph: &Graph,
    config: &DocConfig,
    module: &str,
    modules: &[String],
    urls: &UrlMap,
) -> PageProps {
    // Module pages sit one directory deep (`module/<fqn>.html`): links to the
    // root assets and to other module pages take a `../` prefix.
    let sidebar = build_sidebar(graph, modules, urls, "../");

    let mut nodes: Vec<&GraphNode> = graph
        .nodes()
        .iter()
        .filter(|n| n.module == module && n.kind != NodeKind::Callable)
        .collect();
    nodes.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    let sections = nodes
        .iter()
        .map(|node| build_section(graph, node, urls))
        .collect();

    let page = PageBody::Module(ModuleProps {
        name: module.to_owned(),
        sections,
    });
    PageProps {
        site: site_info(config, "../"),
        sidebar,
        page,
    }
}

/// One node's section: head, docs, tags, relationships, scenarios, diagrams.
fn build_section(graph: &Graph, node: &GraphNode, urls: &UrlMap) -> NodeSection {
    NodeSection {
        id: anchor(&node.fqn),
        kind: node.kind.keyword().to_owned(),
        name: node.name.clone(),
        fqn: node.fqn.clone(),
        visibility: visibility_label(node.visibility).to_owned(),
        summary: node.doc.summary.clone(),
        extended: node.doc.extended.clone(),
        tags: node.doc.tags.clone(),
        relationships: build_relationships(graph, node, urls),
        scenarios: build_scenarios(graph, node),
        diagrams: build_node_diagrams(graph, node),
    }
}

// ---- relationships ---------------------------------------------------------

fn build_relationships(graph: &Graph, node: &GraphNode, urls: &UrlMap) -> Vec<RelGroup> {
    let mut groups = Vec::new();

    if let Some(parent) = &node.parent {
        let edge_kind = if matches!(node.kind, NodeKind::Container | NodeKind::Component) {
            "for"
        } else {
            "in"
        };
        groups.push(RelGroup {
            title: "Parent".to_owned(),
            items: vec![RelItem {
                edge_kind: edge_kind.to_owned(),
                arrow: false,
                fqn: parent.clone(),
                href: urls.href_to(parent),
                label: None,
            }],
        });
    }

    let inbound: Vec<RelItem> = graph
        .edges()
        .iter()
        .filter(|e| e.to == node.fqn && e.kind != EdgeKind::ForParent)
        .map(|edge| rel_item(edge, &edge.from, false, urls))
        .collect();
    if !inbound.is_empty() {
        groups.push(RelGroup {
            title: "Inbound".to_owned(),
            items: inbound,
        });
    }

    let outbound: Vec<RelItem> = graph
        .edges()
        .iter()
        .filter(|e| e.from == node.fqn && e.kind != EdgeKind::ForParent)
        .map(|edge| rel_item(edge, &edge.to, true, urls))
        .collect();
    if !outbound.is_empty() {
        groups.push(RelGroup {
            title: "Outbound".to_owned(),
            items: outbound,
        });
    }

    groups
}

/// A relationship item for `edge`, pointing at `endpoint` (`from` for inbound,
/// `to` for outbound; `arrow` flags the outbound direction).
fn rel_item(edge: &Edge, endpoint: &str, arrow: bool, urls: &UrlMap) -> RelItem {
    RelItem {
        edge_kind: edge_kind_label(edge.kind).to_owned(),
        arrow,
        fqn: endpoint.to_owned(),
        href: urls.href_to(endpoint),
        label: (!edge.label.is_empty()).then(|| edge.label.clone()),
    }
}

// ---- scenarios -------------------------------------------------------------

fn build_scenarios(graph: &Graph, node: &GraphNode) -> Vec<ScenarioCard> {
    graph
        .scenarios_of(&node.fqn)
        .map(|scenario| ScenarioCard {
            name: scenario.name.clone(),
            summary: scenario.doc.summary.clone(),
            extended: scenario.doc.extended.clone(),
            tags: scenario.doc.tags.clone(),
            steps: scenario
                .steps
                .iter()
                .map(|step| Step {
                    keyword: step.keyword.clone(),
                    text: step.text.clone(),
                })
                .collect(),
        })
        .collect()
}

// ---- diagrams --------------------------------------------------------------

/// The diagrams embedded on a node: a C4 sub-view for a system/container, plus a
/// sequence diagram for each triggered callable it owns.
fn build_node_diagrams(graph: &Graph, node: &GraphNode) -> Vec<Diagram> {
    let mut diagrams = Vec::new();

    match node.kind {
        NodeKind::System => diagrams.push(build_diagram(
            graph,
            View::Container {
                of: node.fqn.clone(),
            },
            "Containers",
            "Container diagram",
        )),
        NodeKind::Container => diagrams.push(build_diagram(
            graph,
            View::Component {
                of: node.fqn.clone(),
            },
            "Components",
            "Component diagram",
        )),
        _ => {}
    }

    for callable in callables_of(graph, &node.fqn) {
        if callable.triggers.is_empty() {
            continue;
        }
        diagrams.push(build_diagram(
            graph,
            View::Sequence {
                entry: callable.fqn.clone(),
            },
            "Sequence",
            &format!("Sequence — {}", callable.name),
        ));
    }

    diagrams
}

/// Projects `view` into a [`Diagram`]; an un-projectable view becomes an
/// `Empty` placeholder rather than failing the page.
fn build_diagram(graph: &Graph, view: View, eyebrow: &str, caption: &str) -> Diagram {
    match project(graph, view) {
        Ok(Scene::C4(scene)) => Diagram::C4 {
            caption: caption.to_owned(),
            scene,
        },
        Ok(Scene::Sequence(scene)) => Diagram::Sequence {
            caption: caption.to_owned(),
            scene,
        },
        Err(_) => Diagram::Empty {
            caption: caption.to_owned(),
            eyebrow: eyebrow.to_lowercase(),
        },
    }
}

// ---- sidebar ---------------------------------------------------------------

/// The sidebar tree, with every href prefixed by `prefix` for the page depth.
fn build_sidebar(
    graph: &Graph,
    modules: &[String],
    urls: &UrlMap,
    prefix: &str,
) -> Vec<SidebarModule> {
    modules
        .iter()
        .map(|module| SidebarModule {
            label: module.clone(),
            href: format!("{prefix}{}", module_page_path(module)),
            nodes: module_top_level(graph, module)
                .iter()
                .filter_map(|node| build_sidebar_node(graph, node, urls, prefix))
                .collect(),
        })
        .collect()
}

/// One sidebar node, recursing into its same-module children. `None` when the
/// node has no resolvable URL (it cannot be linked).
fn build_sidebar_node(
    graph: &Graph,
    node: &GraphNode,
    urls: &UrlMap,
    prefix: &str,
) -> Option<SidebarNode> {
    let url = urls.get(&node.fqn)?;
    Some(SidebarNode {
        name: node.name.clone(),
        fqn: node.fqn.clone(),
        kind: node.kind.keyword().to_owned(),
        href: format!("{prefix}{}#{}", url.page, url.anchor),
        children: child_nodes(graph, node)
            .iter()
            .filter_map(|child| build_sidebar_node(graph, child, urls, prefix))
            .collect(),
    })
}

// ---- small label helpers ---------------------------------------------------

fn edge_kind_label(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::ForParent => "for",
        EdgeKind::Call => "call",
        EdgeKind::Trigger => "trigger",
        EdgeKind::Provenance => "from",
    }
}

fn visibility_label(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
    }
}
