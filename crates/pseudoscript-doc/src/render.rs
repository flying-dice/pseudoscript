//! Projects a resolved [`Graph`] into per-page [`PageProps`].
//!
//! Pure and deterministic: modules and nodes are sorted by FQN, callables
//! document under their owner, and same-module children nest. Pages emit in a
//! fixed order — index, authored docs, module pages, then the universe and
//! health pages. Every diagram is pre-rendered SVG under the adaptive palette
//! (the same output as `pds svg`); the client never re-lays-out. No clock, no
//! randomness, no I/O.

use crate::config::{DocConfig, DocPage};
use crate::diag::DiagnosticInput;
use crate::health::{badges_for, build_health};
use crate::highlight::highlight;
use crate::nav::{callables_of, child_nodes, module_top_level, sorted_modules};
use crate::url::{UrlMap, anchor, doc_page_path, module_page_path};
use pseudoscript_emit::{Theme, View, project, render_svg_themed};
use pseudoscript_model::{Edge, EdgeKind, Graph, GraphNode, NodeKind, Visibility};
use pseudoscript_universe::{flows, from_model, snapshot};
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};

use crate::props::{
    Crumb, Diagram, DocPageProps, HealthProps, IndexProps, ModuleCard, ModuleProps, NavLink,
    NodeHref, NodeSection, PageBody, PageProps, RelGroup, RelItem, ScenarioCard, SidebarDocGroup,
    SidebarDocItem, SidebarModule, SidebarNode, SiteInfo, SiteStats, Step, UniverseEdge,
    UniverseNode, UniverseProps,
};

/// The universe page's output path.
pub(crate) const UNIVERSE_PATH: &str = "universe.html";
/// The health page's output path.
pub(crate) const HEALTH_PATH: &str = "health.html";

/// Builds every page's props in deterministic file order: `index.html` first,
/// then one page per authored doc (`[[doc.sidebar]]`, in declaration order),
/// one module page per module sorted by FQN, then the universe and health
/// pages. The returned path is the site-relative output path. `svg_theme`
/// drives the embedded figures: the HTML site always renders adaptively (the
/// `--pds-*` variables recolour at display time); the Markdown site pins its
/// standalone files to the configured scheme.
pub(crate) fn build_pages(
    graph: &Graph,
    config: &DocConfig,
    diagnostics: &[DiagnosticInput],
    svg_theme: Theme,
) -> Vec<(String, PageProps)> {
    let urls = UrlMap::build(graph);
    let modules = sorted_modules(graph);

    let doc_pages: Vec<&DocPage> = config.docs.iter().flat_map(|group| &group.pages).collect();
    let mut pages = Vec::with_capacity(modules.len() + doc_pages.len() + 3);
    pages.push((
        "index.html".to_owned(),
        build_index(graph, config, &modules, &urls, diagnostics, svg_theme),
    ));
    for page in &doc_pages {
        pages.push((
            doc_page_path(&page.path),
            build_doc(config, page, diagnostics),
        ));
    }
    for module in &modules {
        pages.push((
            module_page_path(module),
            build_module(
                graph,
                config,
                module,
                &modules,
                &urls,
                diagnostics,
                svg_theme,
            ),
        ));
    }
    pages.push((
        UNIVERSE_PATH.to_owned(),
        build_universe(graph, config, &modules, &urls, diagnostics),
    ));
    pages.push((
        HEALTH_PATH.to_owned(),
        build_health_page(graph, config, &modules, &urls, diagnostics),
    ));
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

/// The header navigation links: Overview, Universe, and Health with its
/// finding-count badge. `current` marks this page's link.
fn nav_links(diagnostics: &[DiagnosticInput], prefix: &str, current: &str) -> Vec<NavLink> {
    let findings = diagnostics.len();
    [
        ("Overview", "index.html", None),
        ("Universe", UNIVERSE_PATH, None),
        (
            "Health",
            HEALTH_PATH,
            (findings > 0).then(|| findings.to_string()),
        ),
    ]
    .into_iter()
    .map(|(label, path, badge)| NavLink {
        label: label.to_owned(),
        href: format!("{prefix}{path}"),
        badge,
        current: path == current,
    })
    .collect()
}

/// The breadcrumb trail to a page: Overview, then the page's own crumb.
fn crumbs_of(prefix: &str, trail: &[(&str, Option<&str>)]) -> Vec<Crumb> {
    let mut crumbs = vec![Crumb {
        label: "Overview".to_owned(),
        href: Some(format!("{prefix}index.html")),
    }];
    crumbs.extend(trail.iter().map(|(label, href)| Crumb {
        label: (*label).to_owned(),
        href: href.map(|h| format!("{prefix}{h}")),
    }));
    crumbs
}

// ---- authored doc pages ----------------------------------------------------

/// One authored Markdown page (`[[doc.sidebar]]`). Doc pages sit one directory
/// deep (`docs/<slug>.html`), so their sidebar/asset links take a `../` prefix.
fn build_doc(config: &DocConfig, page: &DocPage, diagnostics: &[DiagnosticInput]) -> PageProps {
    PageProps {
        site: site_info(config, "../"),
        doc_groups: build_doc_groups(config, "../"),
        sidebar: Vec::new(),
        nav: nav_links(diagnostics, "../", ""),
        crumbs: crumbs_of("../", &[("Docs", None), (&page.title, None)]),
        page: PageBody::Doc(DocPageProps {
            title: page.title.clone(),
            html: render_markdown(&page.markdown),
        }),
    }
}

/// The authored-doc sidebar groups, every page href prefixed for the page depth.
fn build_doc_groups(config: &DocConfig, prefix: &str) -> Vec<SidebarDocGroup> {
    config
        .docs
        .iter()
        .map(|group| SidebarDocGroup {
            title: group.title.clone(),
            items: group
                .pages
                .iter()
                .map(|page| SidebarDocItem {
                    title: page.title.clone(),
                    href: format!("{prefix}{}", doc_page_path(&page.path)),
                })
                .collect(),
        })
        .collect()
}

/// Renders authored Markdown to HTML. GitHub-flavoured extensions (tables,
/// strikethrough, task lists, footnotes, and `> [!NOTE]`-style alerts) are
/// enabled; fenced code blocks highlight at build time (class-based spans);
/// the content is project-authored and trusted, so raw inline HTML passes
/// through.
fn render_markdown(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_GFM;
    // Rewrite GitHub alert blockquotes into titled callout `<div>`s (matching
    // the IDE live preview) and intercept fenced code blocks for build-time
    // highlighting. A non-alert blockquote passes through unchanged.
    let mut code: Option<(String, String)> = None; // (lang, source) inside a fence
    let events = Parser::new_ext(markdown, options).filter_map(|event| match event {
        Event::Start(Tag::BlockQuote(Some(kind))) => {
            let (slug, label) = callout_meta(kind);
            Some(Event::Html(
                format!("<div class=\"callout callout-{slug}\">\n<p class=\"callout-title\">{label}</p>\n").into(),
            ))
        }
        Event::End(TagEnd::BlockQuote(Some(_))) => Some(Event::Html("</div>\n".into())),
        Event::Start(Tag::CodeBlock(kind)) => {
            let lang = match kind {
                CodeBlockKind::Fenced(lang) => lang.split_whitespace().next().unwrap_or("").to_owned(),
                CodeBlockKind::Indented => String::new(),
            };
            code = Some((lang, String::new()));
            None
        }
        Event::Text(text) if code.is_some() => {
            if let Some((_, source)) = code.as_mut() {
                source.push_str(&text);
            }
            None
        }
        Event::End(TagEnd::CodeBlock) => {
            let (lang, source) = code.take().unwrap_or_default();
            let classes = if lang.is_empty() {
                "code-block".to_owned()
            } else {
                format!("code-block language-{lang}")
            };
            Some(Event::Html(
                format!(
                    "<pre class=\"{classes}\"><code>{}</code></pre>\n",
                    highlight(&lang, &source)
                )
                .into(),
            ))
        }
        other => Some(other),
    });
    let mut html = String::with_capacity(markdown.len() * 3 / 2);
    html::push_html(&mut html, events);
    html
}

/// The CSS slug and display title for a GitHub alert kind.
fn callout_meta(kind: BlockQuoteKind) -> (&'static str, &'static str) {
    match kind {
        BlockQuoteKind::Note => ("note", "Note"),
        BlockQuoteKind::Tip => ("tip", "Tip"),
        BlockQuoteKind::Important => ("important", "Important"),
        BlockQuoteKind::Warning => ("warning", "Warning"),
        BlockQuoteKind::Caution => ("caution", "Caution"),
    }
}

// ---- index ----------------------------------------------------------------

fn build_index(
    graph: &Graph,
    config: &DocConfig,
    modules: &[String],
    urls: &UrlMap,
    diagnostics: &[DiagnosticInput],
    svg_theme: Theme,
) -> PageProps {
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
        context_diagram: build_svg_diagram(
            graph,
            View::Context,
            "Context",
            "System context",
            svg_theme,
        ),
        cards,
        stats: build_stats(graph, diagnostics),
    });
    PageProps {
        site: site_info(config, ""),
        doc_groups: build_doc_groups(config, ""),
        sidebar,
        nav: nav_links(diagnostics, "", "index.html"),
        crumbs: Vec::new(), // the index IS the root; no trail
        page,
    }
}

/// The stats strip: structural node counts, the flow count, the finding count.
fn build_stats(graph: &Graph, diagnostics: &[DiagnosticInput]) -> SiteStats {
    let count = |kind: NodeKind| graph.nodes().iter().filter(|n| n.kind == kind).count();
    SiteStats {
        systems: count(NodeKind::System),
        containers: count(NodeKind::Container),
        components: count(NodeKind::Component),
        flows: flows(graph).len(),
        findings: diagnostics.len(),
    }
}

// ---- universe ---------------------------------------------------------------

/// The universe page: the flat snapshot, the traced flows, and each placed
/// node's documentation href — everything the 3D island draws.
fn build_universe(
    graph: &Graph,
    config: &DocConfig,
    modules: &[String],
    urls: &UrlMap,
    diagnostics: &[DiagnosticInput],
) -> PageProps {
    let snap = snapshot(&from_model(graph));
    let hrefs = snap
        .nodes
        .iter()
        .filter_map(|n| {
            urls.get(&n.id).map(|url| NodeHref {
                id: n.id.clone(),
                href: format!("{}#{}", url.page, url.anchor),
            })
        })
        .collect();
    let page = PageBody::Universe(UniverseProps {
        nodes: snap
            .nodes
            .into_iter()
            .map(|n| UniverseNode {
                id: n.id,
                level: n.level.to_owned(),
                parent: n.parent,
            })
            .collect(),
        edges: snap
            .edges
            .into_iter()
            .map(|e| UniverseEdge {
                from: e.from,
                to: e.to,
                traffic: e.traffic,
            })
            .collect(),
        flows: flows(graph),
        hrefs,
    });
    PageProps {
        site: site_info(config, ""),
        doc_groups: build_doc_groups(config, ""),
        sidebar: build_sidebar(graph, modules, urls, ""),
        nav: nav_links(diagnostics, "", UNIVERSE_PATH),
        crumbs: crumbs_of("", &[("Universe", None)]),
        page,
    }
}

// ---- health -----------------------------------------------------------------

fn build_health_page(
    graph: &Graph,
    config: &DocConfig,
    modules: &[String],
    urls: &UrlMap,
    diagnostics: &[DiagnosticInput],
) -> PageProps {
    let health: HealthProps = build_health(graph, diagnostics, urls, "");
    PageProps {
        site: site_info(config, ""),
        doc_groups: build_doc_groups(config, ""),
        sidebar: build_sidebar(graph, modules, urls, ""),
        nav: nav_links(diagnostics, "", HEALTH_PATH),
        crumbs: crumbs_of("", &[("Health", None)]),
        page: PageBody::Health(health),
    }
}

// ---- module page -----------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn build_module(
    graph: &Graph,
    config: &DocConfig,
    module: &str,
    modules: &[String],
    urls: &UrlMap,
    diagnostics: &[DiagnosticInput],
    svg_theme: Theme,
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
        .map(|node| build_section(graph, node, urls, diagnostics, svg_theme))
        .collect();

    let page = PageBody::Module(ModuleProps {
        name: module.to_owned(),
        sections,
    });
    PageProps {
        site: site_info(config, "../"),
        doc_groups: build_doc_groups(config, "../"),
        sidebar,
        nav: nav_links(diagnostics, "../", ""),
        crumbs: crumbs_of("../", &[("Modules", None), (module, None)]),
        page,
    }
}

/// One node's section: head, docs, tags, relationships, scenarios, diagrams,
/// and the health findings attributed to it.
fn build_section(
    graph: &Graph,
    node: &GraphNode,
    urls: &UrlMap,
    diagnostics: &[DiagnosticInput],
    svg_theme: Theme,
) -> NodeSection {
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
        scenarios: build_scenarios(graph, node, svg_theme),
        diagrams: build_node_diagrams(graph, node, svg_theme),
        diagnostics: badges_for(graph, node, diagnostics),
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

fn build_scenarios(graph: &Graph, node: &GraphNode, svg_theme: Theme) -> Vec<ScenarioCard> {
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
            flow: build_svg_diagram(
                graph,
                View::Feature {
                    of: format!("{}::{}", scenario.module, scenario.name),
                },
                "Flow",
                &format!("Flow — {}", scenario.name),
                svg_theme,
            ),
        })
        .collect()
}

// ---- diagrams --------------------------------------------------------------

/// The diagrams embedded on a node: a C4 sub-view for a system/container, an
/// entity view for a `data` type (`LANG.md` §9.4), plus a sequence diagram for
/// each triggered callable it owns.
fn build_node_diagrams(graph: &Graph, node: &GraphNode, svg_theme: Theme) -> Vec<Diagram> {
    let mut diagrams = Vec::new();

    match node.kind {
        NodeKind::System => diagrams.push(build_svg_diagram(
            graph,
            View::Container {
                of: node.fqn.clone(),
            },
            "Containers",
            "Container diagram",
            svg_theme,
        )),
        NodeKind::Container => diagrams.push(build_svg_diagram(
            graph,
            View::Component {
                of: node.fqn.clone(),
            },
            "Components",
            "Component diagram",
            svg_theme,
        )),
        NodeKind::Data => diagrams.push(build_svg_diagram(
            graph,
            View::Data {
                of: node.fqn.clone(),
            },
            "Entities",
            "Entity diagram",
            svg_theme,
        )),
        _ => {}
    }

    for callable in callables_of(graph, &node.fqn) {
        if callable.triggers.is_empty() {
            continue;
        }
        diagrams.push(build_svg_diagram(
            graph,
            View::Sequence {
                entry: callable.fqn.clone(),
            },
            "Sequence",
            &format!("Sequence — {}", callable.name),
            svg_theme,
        ));
    }

    diagrams
}

/// The `data-diagram` kind word for a view.
fn view_kind(view: &View) -> &'static str {
    match view {
        View::Context | View::Container { .. } | View::Component { .. } => "c4",
        View::Sequence { .. } => "sequence",
        View::Data { .. } => "entity",
        View::Feature { .. } => "flow",
    }
}

/// Projects `view` and pre-renders it to deterministic SVG under the adaptive
/// palette — the same output as `pds svg`. An un-projectable view becomes an
/// `Empty` placeholder rather than failing the page.
fn build_svg_diagram(
    graph: &Graph,
    view: View,
    eyebrow: &str,
    caption: &str,
    svg_theme: Theme,
) -> Diagram {
    let kind = view_kind(&view);
    match project(graph, view) {
        Ok(scene) => Diagram::Svg {
            caption: caption.to_owned(),
            eyebrow: eyebrow.to_owned(),
            kind: kind.to_owned(),
            svg: render_svg_themed(&scene, svg_theme),
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

#[cfg(test)]
mod tests {
    use super::{build_pages, render_markdown};
    use crate::config::{DocConfig, DocGroup, DocPage};
    use crate::props::{Diagram, PageBody};
    use pseudoscript_model::{WorkspaceModule, graph};

    fn config_with_docs() -> DocConfig {
        DocConfig {
            docs: vec![DocGroup {
                title: "Getting Started".to_owned(),
                pages: vec![DocPage {
                    title: "Introduction".to_owned(),
                    path: "docs/introduction.md".to_owned(),
                    markdown: "# Hi\n\nSome **bold** text.".to_owned(),
                }],
            }],
            ..DocConfig::default()
        }
    }

    #[test]
    fn pages_emit_in_order_index_docs_modules_universe_health() {
        let g = graph(&[WorkspaceModule::new("m", "//! m\npublic system S;")]);
        let pages = build_pages(
            &g,
            &config_with_docs(),
            &[],
            pseudoscript_emit::Theme::Adaptive,
        );
        let paths: Vec<&str> = pages.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths[0], "index.html");
        assert_eq!(paths[1], "docs/introduction.html");
        assert!(paths.iter().any(|p| p.starts_with("module/")));
        assert_eq!(paths[paths.len() - 2], "universe.html");
        assert_eq!(paths[paths.len() - 1], "health.html");
    }

    #[test]
    fn doc_page_carries_rendered_markdown_and_sidebar_group() {
        let g = graph(&[WorkspaceModule::new("m", "//! m\npublic system S;")]);
        let pages = build_pages(
            &g,
            &config_with_docs(),
            &[],
            pseudoscript_emit::Theme::Adaptive,
        );
        let (_, doc) = pages
            .iter()
            .find(|(p, _)| p == "docs/introduction.html")
            .expect("doc page present");
        // The body is rendered HTML; the sidebar group links it with a `../` prefix.
        let PageBody::Doc(body) = &doc.page else {
            panic!("expected a doc page body");
        };
        assert!(body.html.contains("<strong>bold</strong>"), "{}", body.html);
        let group = &doc.doc_groups[0];
        assert_eq!(group.title, "Getting Started");
        assert_eq!(group.items[0].href, "../docs/introduction.html");
    }

    #[test]
    fn every_diagram_is_adaptive_inline_svg() {
        let src = "//! m\n/// Sys.\npublic system S;\npublic container C for m::S {\n  #[manual]\n  public Run() {\n    Step()\n  }\n  Step();\n}\n";
        let g = graph(&[WorkspaceModule::new("m", src)]);
        let pages = build_pages(
            &g,
            &DocConfig::default(),
            &[],
            pseudoscript_emit::Theme::Adaptive,
        );
        let (_, module) = pages
            .iter()
            .find(|(p, _)| p.starts_with("module/"))
            .expect("module page");
        let PageBody::Module(body) = &module.page else {
            panic!("expected a module page body");
        };
        let diagrams: Vec<&Diagram> = body.sections.iter().flat_map(|s| &s.diagrams).collect();
        assert!(!diagrams.is_empty());
        for diagram in diagrams {
            if let Diagram::Svg { svg, kind, .. } = diagram {
                assert!(svg.starts_with("<svg"), "inline SVG, kind {kind}");
                assert!(svg.contains("var(--pds-"), "adaptive palette");
            }
        }
    }

    #[test]
    fn universe_page_carries_snapshot_flows_and_hrefs() {
        let src = "//! m\npublic system S;\npublic container C for m::S {\n  #[manual]\n  public Run() {\n    m::D.Go()\n  }\n}\npublic container D for m::S {\n  public Go();\n}\n";
        let g = graph(&[WorkspaceModule::new("m", src)]);
        let pages = build_pages(
            &g,
            &DocConfig::default(),
            &[],
            pseudoscript_emit::Theme::Adaptive,
        );
        let (_, universe) = pages
            .iter()
            .find(|(p, _)| p == "universe.html")
            .expect("universe page");
        let PageBody::Universe(body) = &universe.page else {
            panic!("expected the universe page body");
        };
        assert_eq!(body.nodes.len(), 3, "system + two containers");
        assert!(!body.edges.is_empty());
        assert_eq!(body.flows.len(), 1);
        assert!(body.hrefs.iter().any(|h| h.id == "m::C"));
    }

    #[test]
    fn markdown_renders_gfm_tables() {
        let html = render_markdown("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<table>"), "{html}");
    }

    #[test]
    fn markdown_renders_github_alerts_as_titled_callouts() {
        let html = render_markdown("> [!WARNING]\n> Be careful.");
        assert!(
            html.contains(r#"<div class="callout callout-warning">"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<p class="callout-title">Warning</p>"#),
            "{html}"
        );
        assert!(html.contains("Be careful."), "{html}");
        // The `[!WARNING]` marker is consumed, not rendered as content.
        assert!(!html.contains("[!WARNING]"), "{html}");
    }

    #[test]
    fn markdown_plain_blockquote_stays_a_blockquote() {
        let html = render_markdown("> just a quote");
        assert!(html.contains("<blockquote>"), "{html}");
        assert!(!html.contains("callout"), "{html}");
    }

    #[test]
    fn markdown_highlights_pds_fences_and_escapes_others() {
        let html = render_markdown("```pds\npublic system S;\n```\n\n```js\nlet a = 1 < 2;\n```");
        assert!(
            html.contains(r#"<span class="tok-kw">public</span>"#),
            "{html}"
        );
        assert!(html.contains("language-js"), "{html}");
        assert!(html.contains("1 &lt; 2"), "{html}");
    }
}
