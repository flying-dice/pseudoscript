//! Site rendering: the resolved [`Graph`] → a [`Site`] of HTML/CSS/JS files.
//!
//! Generation is a pure, deterministic projection (`LANG.md` §9.3): modules and
//! nodes are sorted by FQN, no clock or randomness is consulted, and every
//! piece of user text is escaped through [`crate::escape`]. The output is
//! `index.html`, the shared `style.css` / `app.js`, and one page per module.

use std::fmt::Write as _;

use pseudoscript_emit::{View, project, render_svg};
use pseudoscript_model::{Edge, EdgeKind, Graph, GraphNode, NodeKind, Visibility};

use crate::assets::{APP_JS, STYLE_CSS};
use crate::config::DocConfig;
use crate::escape::escape;
use crate::nav::{callables_of, child_nodes, module_top_level, sorted_modules};
use crate::site::{Site, SiteFile};
use crate::url::{UrlMap, anchor, module_page_path};

/// Renders the whole documentation site for `graph` under `config`.
#[must_use]
pub fn render_site(graph: &Graph, config: &DocConfig) -> Site {
    let urls = UrlMap::build(graph);
    let modules = sorted_modules(graph);
    let sidebar = render_sidebar(graph, &modules, &urls);

    let mut files = Vec::with_capacity(modules.len() + 3);
    files.push(SiteFile::new(
        "index.html",
        render_index(graph, config, &modules, &sidebar),
    ));
    files.push(SiteFile::new("style.css", STYLE_CSS));
    files.push(SiteFile::new("app.js", APP_JS));
    for module in &modules {
        files.push(SiteFile::new(
            module_page_path(module),
            render_module_page(graph, config, module, &urls, &sidebar),
        ));
    }
    Site { files }
}

// ---- page shell -----------------------------------------------------------

/// Wraps page `body` in the shared HTML shell (head, theme attr, sidebar, asset
/// links). `depth` is how many directories deep the page sits, fixing the
/// relative prefix to the root-level assets.
fn page_shell(config: &DocConfig, depth: usize, sidebar: &str, body: &str) -> String {
    let prefix = "../".repeat(depth);
    let title = escape(&config.name);
    let mut html = String::with_capacity(body.len() + sidebar.len() + 1024);
    let _ = write!(
        html,
        "<!doctype html>\n\
<html lang=\"en\" data-theme=\"{theme}\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title}</title>\n\
<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n\
<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n\
<link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?\
family=Bricolage+Grotesque:opsz,wght@12..96,600;12..96,700&\
family=Hanken+Grotesk:wght@400;500;600&\
family=JetBrains+Mono:wght@400;500;600&display=swap\">\n\
<link rel=\"stylesheet\" href=\"{prefix}style.css\">\n\
</head>\n\
<body>\n\
<div class=\"layout\">\n\
{sidebar}\
<main class=\"main\">\n\
{body}\
</main>\n\
</div>\n\
<script src=\"{prefix}app.js\"></script>\n\
</body>\n\
</html>\n",
        theme = config.theme.attr(),
    );
    html
}

/// The brand + search + tree sidebar, identical on every page (paths use the
/// page-depth-agnostic module-page form, which is correct from both `index.html`
/// — via the relative prefix — and sibling module pages).
///
/// The tree mirrors the C4 hierarchy: each module holds its top-level nodes, and
/// every node nests its same-module children (a `system`'s containers, a
/// container's components, a node's nested declarations) recursively.
fn render_sidebar(graph: &Graph, modules: &[String], urls: &UrlMap) -> String {
    let mut tree = String::new();
    for module in modules {
        let _ = write!(
            tree,
            "<li class=\"module\" data-search=\"{search}\">\
<div class=\"row\"><span class=\"toggle\">&#9662;</span>\
<a class=\"label\" href=\"PREFIX{href}\">{label}</a></div>\
<ul class=\"children\">",
            search = escape(module),
            href = module_page_path(module),
            label = escape(module),
        );
        for node in module_top_level(graph, module) {
            render_tree_node(graph, node, urls, &mut tree);
        }
        tree.push_str("</ul></li>");
    }
    format!(
        "<aside class=\"sidebar\">\
<div class=\"brand\">BRAND</div>\
<div class=\"search\"><input type=\"search\" placeholder=\"Filter nodes…\" \
aria-label=\"Filter nodes\" autocomplete=\"off\" spellcheck=\"false\"></div>\
<ul class=\"tree\">{tree}</ul>\
</aside>\n",
    )
}

/// Renders one node as a tree `<li>`, recursing into its same-module children.
/// A node with children carries a collapse toggle; a leaf gets a spacer so its
/// label aligns with its siblings'.
fn render_tree_node(graph: &Graph, node: &GraphNode, urls: &UrlMap, out: &mut String) {
    let Some(url) = urls.get(&node.fqn) else {
        return;
    };
    let children = child_nodes(graph, node);
    let toggle = if children.is_empty() {
        "<span class=\"toggle-spacer\"></span>"
    } else {
        "<span class=\"toggle\">&#9662;</span>"
    };
    let _ = write!(
        out,
        "<li data-search=\"{search}\"><div class=\"row\">{toggle}\
<a class=\"node-link\" href=\"PREFIX{page}#{anchor}\">\
<span class=\"kind-dot {kind}\"></span>\
<span class=\"label\">{name}</span></a></div>",
        search = escape(&node.fqn),
        page = url.page,
        anchor = url.anchor,
        kind = node.kind.keyword(),
        name = escape(&node.name),
    );
    if !children.is_empty() {
        out.push_str("<ul class=\"children\">");
        for child in children {
            render_tree_node(graph, child, urls, out);
        }
        out.push_str("</ul>");
    }
    out.push_str("</li>");
}

/// Resolves the `PREFIX`/`BRAND` placeholders in the shared sidebar for a page
/// at `depth`. Keeping the tree string depth-agnostic lets us build it once.
fn localize_sidebar(sidebar: &str, config: &DocConfig, depth: usize) -> String {
    let prefix = "../".repeat(depth);
    let brand = render_brand(config, depth);
    sidebar.replace("PREFIX", &prefix).replace("BRAND", &brand)
}

/// The brand block: optional logo image plus the site title.
fn render_brand(config: &DocConfig, depth: usize) -> String {
    let prefix = "../".repeat(depth);
    let logo = config.logo_filename().map_or(String::new(), |file| {
        format!("<img src=\"{prefix}{file}\" alt=\"\">", file = escape(file))
    });
    format!(
        "<a class=\"brand-link\" href=\"{prefix}index.html\">\
{logo}<span class=\"title\">{name}<small>PseudoScript</small></span></a>",
        name = escape(&config.name),
    )
}

// ---- index ----------------------------------------------------------------

/// The index page: title, the context diagram, and module cards.
fn render_index(graph: &Graph, config: &DocConfig, modules: &[String], sidebar: &str) -> String {
    let mut body = String::new();
    let _ = write!(
        body,
        "<div class=\"content\">\
<header class=\"page-head\">\
<div class=\"eyebrow\">Architecture documentation</div>\
<h1>{name}</h1>\
<p class=\"lead\">A C4 model of the workspace: persons, systems, and their \
containers and components, with relationships and sequence flows.</p>\
</header>",
        name = escape(&config.name),
    );

    body.push_str(&render_diagram(
        graph,
        View::Context,
        "Context",
        "System context — persons and systems",
    ));

    body.push_str("<section class=\"card-grid\">");
    for module in modules {
        let count = module_top_level(graph, module).len();
        let _ = write!(
            body,
            "<a class=\"card\" href=\"{href}\">\
<div class=\"card-title\">{name}</div>\
<div class=\"card-meta\">{count} item{plural}</div></a>",
            href = module_page_path(module),
            name = escape(module),
            plural = if count == 1 { "" } else { "s" },
        );
    }
    body.push_str("</section>");

    body.push_str(&render_footer());
    body.push_str("</div>\n");

    let sidebar = localize_sidebar(sidebar, config, 0);
    page_shell(config, 0, &sidebar, &body)
}

// ---- module page ----------------------------------------------------------

/// A module's page: header plus a section per node declared in it.
fn render_module_page(
    graph: &Graph,
    config: &DocConfig,
    module: &str,
    urls: &UrlMap,
    sidebar: &str,
) -> String {
    let mut body = String::new();
    let _ = write!(
        body,
        "<div class=\"content\">\
<header class=\"page-head\">\
<div class=\"eyebrow\">Module</div>\
<h1 class=\"mono\">{name}</h1></header>",
        name = escape(module),
    );

    // Every node declared in this module gets a section, top-level first then
    // nested, all sorted by FQN, so every anchor a cross-link points at exists.
    let mut nodes: Vec<&GraphNode> = graph
        .nodes()
        .iter()
        .filter(|n| n.module == module && n.kind != NodeKind::Callable)
        .collect();
    nodes.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    for node in nodes {
        body.push_str(&render_node(graph, node, urls));
    }

    body.push_str(&render_footer());
    body.push_str("</div>\n");

    let sidebar = localize_sidebar(sidebar, config, 1);
    page_shell(config, 1, &sidebar, &body)
}

/// One node section: head (kind/visibility badges, FQN), docs, tags,
/// relationships, and any embedded diagram(s).
fn render_node(graph: &Graph, node: &GraphNode, urls: &UrlMap) -> String {
    let id = anchor(&node.fqn);
    let mut html = String::new();
    let _ = write!(
        html,
        "<section class=\"node\" id=\"{id}\">\
<div class=\"node-head\">\
<span class=\"kind-badge {kind}\">{kind}</span>\
<h2><a href=\"#{id}\">{name}</a> \
<span class=\"self-link\">#</span></h2>\
<span class=\"vis-badge\">{vis}</span>\
</div>\
<code class=\"node-fqn\">{fqn}</code>",
        kind = node.kind.keyword(),
        name = escape(&node.name),
        vis = visibility_label(node.visibility),
        fqn = escape(&node.fqn),
    );

    if let Some(summary) = &node.doc.summary {
        let _ = write!(html, "<p class=\"summary\">{}</p>", escape(summary));
    }
    if let Some(extended) = &node.doc.extended {
        let _ = write!(html, "<p class=\"extended\">{}</p>", escape(extended));
    }
    if !node.doc.tags.is_empty() {
        html.push_str("<div class=\"tags\">");
        for tag in &node.doc.tags {
            let _ = write!(html, "<span class=\"chip\">{}</span>", escape(tag));
        }
        html.push_str("</div>");
    }

    html.push_str(&render_relationships(graph, node, urls));
    html.push_str(&render_node_scenarios(graph, node));
    html.push_str(&render_node_diagrams(graph, node));

    html.push_str("</section>");
    html
}

/// The BDD scenario cards for a node: each `feature` targeting it, rendered as
/// its given/when/then steps (`LANG.md` §5.2, §9.3). Empty when the node has no
/// features.
fn render_node_scenarios(graph: &Graph, node: &GraphNode) -> String {
    let scenarios: Vec<_> = graph.scenarios_of(&node.fqn).collect();
    if scenarios.is_empty() {
        return String::new();
    }
    let mut out = String::from("<div class=\"scenarios\"><h3>Scenarios</h3>");
    for scenario in scenarios {
        let _ = write!(
            out,
            "<div class=\"scenario\"><div class=\"scenario-name\">{name}</div>",
            name = escape(&scenario.name),
        );
        if let Some(summary) = &scenario.doc.summary {
            let _ = write!(out, "<p class=\"summary\">{}</p>", escape(summary));
        }
        if let Some(extended) = &scenario.doc.extended {
            let _ = write!(out, "<p class=\"extended\">{}</p>", escape(extended));
        }
        if !scenario.doc.tags.is_empty() {
            out.push_str("<div class=\"tags\">");
            for tag in &scenario.doc.tags {
                let _ = write!(out, "<span class=\"chip\">{}</span>", escape(tag));
            }
            out.push_str("</div>");
        }
        out.push_str("<ul class=\"steps\">");
        for step in &scenario.steps {
            // The keyword is a closed set (given/when/then/and/but), safe as a
            // class and label; the prose is user text and is escaped.
            let _ = write!(
                out,
                "<li><span class=\"step-kw {kw}\">{kw}</span> \
<span class=\"step-text\">{text}</span></li>",
                kw = step.keyword,
                text = escape(&step.text),
            );
        }
        out.push_str("</ul></div>");
    }
    out.push_str("</div>");
    out
}

/// The relationships block: the `for`/owner parent, inbound edges, and outbound
/// edges (`LANG.md` §9.3). Each endpoint is a cross-link when it resolves.
fn render_relationships(graph: &Graph, node: &GraphNode, urls: &UrlMap) -> String {
    let mut groups = String::new();

    if let Some(parent) = &node.parent {
        let label = if matches!(node.kind, NodeKind::Container | NodeKind::Component) {
            "for"
        } else {
            "in"
        };
        let _ = write!(
            groups,
            "<div class=\"rel-group\"><h3>Parent</h3><ul class=\"rel-list\">\
<li><span class=\"edge-kind\">{label}</span>{link}</li></ul></div>",
            link = fqn_link(parent, urls),
        );
    }

    let inbound: Vec<&Edge> = graph
        .edges()
        .iter()
        .filter(|e| e.to == node.fqn && e.kind != EdgeKind::ForParent)
        .collect();
    if !inbound.is_empty() {
        groups.push_str("<div class=\"rel-group\"><h3>Inbound</h3><ul class=\"rel-list\">");
        for edge in inbound {
            let _ = write!(
                groups,
                "<li><span class=\"edge-kind\">{kind}</span>{from}{label}</li>",
                kind = edge_kind_label(edge.kind),
                from = fqn_link(&edge.from, urls),
                label = edge_label(&edge.label),
            );
        }
        groups.push_str("</ul></div>");
    }

    let outbound: Vec<&Edge> = graph
        .edges()
        .iter()
        .filter(|e| e.from == node.fqn && e.kind != EdgeKind::ForParent)
        .collect();
    if !outbound.is_empty() {
        groups.push_str("<div class=\"rel-group\"><h3>Outbound</h3><ul class=\"rel-list\">");
        for edge in outbound {
            let _ = write!(
                groups,
                "<li><span class=\"edge-kind\">{kind}</span>\
<span class=\"arrow\">&rarr;</span> {to}{label}</li>",
                kind = edge_kind_label(edge.kind),
                to = fqn_link(&edge.to, urls),
                label = edge_label(&edge.label),
            );
        }
        groups.push_str("</ul></div>");
    }

    if groups.is_empty() {
        return String::new();
    }
    format!("<div class=\"rel\">{groups}</div>")
}

/// The embedded diagrams for a node: a container diagram on a `system`, a
/// component diagram on a `container`, and a sequence diagram per triggered
/// callable the node owns. A view that fails to project is skipped gracefully.
fn render_node_diagrams(graph: &Graph, node: &GraphNode) -> String {
    let mut out = String::new();

    match node.kind {
        NodeKind::System => out.push_str(&render_diagram(
            graph,
            View::Container {
                of: node.fqn.clone(),
            },
            "Containers",
            "Container diagram",
        )),
        NodeKind::Container => out.push_str(&render_diagram(
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
        let caption = format!("Sequence — {}", callable.name);
        out.push_str(&render_diagram(
            graph,
            View::Sequence {
                entry: callable.fqn.clone(),
            },
            "Sequence",
            &caption,
        ));
    }

    out
}

// ---- diagram figure --------------------------------------------------------

/// A framed, zoom/pan-able diagram figure for `view`. Renders the inline SVG on
/// success; on an [`EmitError`](pseudoscript_emit::EmitError) (e.g. an empty
/// boundary) it emits a graceful placeholder instead.
fn render_diagram(graph: &Graph, view: View, eyebrow: &str, caption: &str) -> String {
    match project(graph, view) {
        Ok(scene) => format!(
            "<figure class=\"figure\">\
<figcaption><span class=\"cap-title\">{caption}</span>\
<span class=\"hint\">scroll to zoom · drag to pan</span></figcaption>\
<div class=\"diagram\">\
<div class=\"controls\">\
<button type=\"button\" class=\"fs-toggle\" aria-label=\"Toggle fullscreen\" \
title=\"Fullscreen\">&#9974;</button>\
<button type=\"button\" class=\"zoom-reset\">reset</button>\
</div>\
<div class=\"pan\">{svg}</div></div></figure>",
            caption = escape(caption),
            svg = render_svg(&scene),
        ),
        Err(_) => format!(
            "<div class=\"no-diagram\">No {eyebrow} diagram available.</div>",
            eyebrow = escape(eyebrow).to_lowercase(),
        ),
    }
}

// ---- small helpers ---------------------------------------------------------

/// A cross-link to `fqn` (a module-page-relative `href`), or the plain escaped
/// FQN when it resolves to no node (`LANG.md` §9.3).
fn fqn_link(fqn: &str, urls: &UrlMap) -> String {
    match urls.href_to(fqn) {
        Some(href) => format!("<a class=\"fqn\" href=\"{href}\">{}</a>", escape(fqn)),
        None => format!("<span class=\"fqn\">{}</span>", escape(fqn)),
    }
}

/// The trailing `· method` label for a `Call` edge, empty otherwise.
fn edge_label(label: &str) -> String {
    if label.is_empty() {
        String::new()
    } else {
        format!(" <span class=\"edge-label\">· {}</span>", escape(label))
    }
}

/// The display word for an [`EdgeKind`].
fn edge_kind_label(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::ForParent => "for",
        EdgeKind::Call => "call",
        EdgeKind::Trigger => "trigger",
        EdgeKind::Provenance => "from",
    }
}

/// The display word for a [`Visibility`].
fn visibility_label(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
    }
}

/// The shared page footer.
fn render_footer() -> String {
    "<footer class=\"foot\">Generated by <code>pds doc</code>.</footer>".to_owned()
}
