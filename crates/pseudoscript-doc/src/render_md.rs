//! Markdown documentation renderer: one `.md` per page, with each diagram
//! written as a **standalone `.svg` file** and referenced (`![caption](…)`).
//!
//! Inline `<svg>` inside Markdown is dropped or sanitised by many renderers
//! (GitHub, IDE previews); a referenced `.svg` image renders everywhere, text
//! included. The same page model ([`build_pages`](crate::render::build_pages))
//! rendered to Markdown instead of driven through SSR — no JavaScript, every
//! target. Diagrams use the workspace's configured theme (`[doc].theme`), so the
//! Markdown and HTML outputs match.

use std::fmt::Write as _;

use pseudoscript_emit::{Theme, adaptive_style_block};

use crate::props::{
    Diagram, DocPageProps, HealthProps, IndexProps, ModuleProps, NodeSection, PageBody, PageProps,
    RelGroup, ScenarioCard, UniverseProps,
};
use crate::site::SiteFile;

/// State threaded through one page: the diagram theme (drives the adaptive
/// style block on standalone files), the page's `../`-to-root prefix (so an
/// image link resolves to the site-root `diagrams/` dir), and the collector the
/// standalone `.svg` files are pushed onto.
struct Ctx<'a> {
    theme: Theme,
    prefix: &'a str,
    svgs: &'a mut Vec<SiteFile>,
}

/// Renders the built pages to Markdown `SiteFile`s plus the `diagrams/*.svg`
/// files they reference. `theme` drives the diagrams.
pub(crate) fn pages_to_markdown(pages: &[(String, PageProps)], theme: Theme) -> Vec<SiteFile> {
    let mut svgs: Vec<SiteFile> = Vec::new();
    let mut files: Vec<SiteFile> = Vec::with_capacity(pages.len());
    for (path, props) in pages {
        let mut ctx = Ctx {
            theme,
            prefix: &props.site.prefix,
            svgs: &mut svgs,
        };
        files.push(SiteFile::new(md_path(path), render_page(props, &mut ctx)));
    }
    files.append(&mut svgs);
    files
}

/// Swaps a page's `.html` extension for `.md`, leaving its directory intact.
fn md_path(html_path: &str) -> String {
    html_path
        .strip_suffix(".html")
        .map_or_else(|| html_path.to_owned(), |stem| format!("{stem}.md"))
}

/// Rewrites an in-site `.html` href (with an optional `#anchor`) to its `.md`
/// target so cross-links resolve between the generated Markdown files.
fn md_href(href: &str) -> String {
    let (path, anchor) = match href.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (href, None),
    };
    let path = path
        .strip_suffix(".html")
        .map_or_else(|| path.to_owned(), |stem| format!("{stem}.md"));
    match anchor {
        Some(anchor) => format!("{path}#{anchor}"),
        None => path,
    }
}

/// Renders one page's body to Markdown, collecting its diagrams into `ctx`.
fn render_page(props: &PageProps, ctx: &mut Ctx) -> String {
    let mut out = String::new();
    match &props.page {
        PageBody::Index(index) => render_index(&mut out, index, ctx),
        PageBody::Module(module) => render_module(&mut out, module, ctx),
        PageBody::Doc(doc) => render_doc(&mut out, doc),
        PageBody::Universe(universe) => render_universe(&mut out, universe),
        PageBody::Health(health) => render_health(&mut out, health),
    }
    out
}

/// The universe as text lists: nodes by level, edges with traffic, each flow's
/// legs — the engine-free reading of the 3D page.
fn render_universe(out: &mut String, universe: &UniverseProps) {
    out.push_str("# Universe\n\n");
    if !universe.nodes.is_empty() {
        out.push_str("## Nodes\n\n");
        for node in &universe.nodes {
            let _ = writeln!(out, "- `{}` — {}", node.id, node.level);
        }
        out.push('\n');
    }
    if !universe.edges.is_empty() {
        out.push_str("## Relationships\n\n");
        for edge in &universe.edges {
            let _ = writeln!(out, "- `{}` \u{2192} `{}` ({} call{})", edge.from, edge.to, edge.traffic, if edge.traffic == 1 { "" } else { "s" });
        }
        out.push('\n');
    }
    if !universe.flows.is_empty() {
        out.push_str("## Flows\n\n");
        for flow in &universe.flows {
            let _ = writeln!(out, "- **{}** (`{}`)", flow.name, flow.fqn);
            for hop in &flow.hops {
                let _ = writeln!(out, "  - `{}` \u{2192} `{}` — {}", hop.from, hop.to, hop.label);
            }
        }
        out.push('\n');
    }
}

/// The health report as a table: severity, code (linked to its article),
/// location, message, and the owning node.
fn render_health(out: &mut String, health: &HealthProps) {
    out.push_str("# Architecture health\n\n");
    let _ = writeln!(
        out,
        "{} error{}, {} warning{}\n",
        health.error_count,
        if health.error_count == 1 { "" } else { "s" },
        health.warning_count,
        if health.warning_count == 1 { "" } else { "s" },
    );
    if health.entries.is_empty() {
        out.push_str("No findings.\n\n");
        return;
    }
    out.push_str("| Severity | Code | Location | Message | Node |\n|---|---|---|---|---|\n");
    for entry in &health.entries {
        let code = match (&entry.code, &entry.code_url) {
            (Some(code), Some(url)) => format!("[{code}]({url})"),
            (Some(code), None) => code.clone(),
            _ => String::new(),
        };
        let node = if entry.node_fqn.is_empty() {
            String::new()
        } else {
            format!("[{}]({})", entry.node_fqn, md_href(&entry.href))
        };
        let _ = writeln!(
            out,
            "| {} | {} | {}:{}:{} | {} | {} |",
            entry.severity,
            code,
            entry.module,
            entry.line,
            entry.column,
            entry.message.replace('|', "\\|"),
            node,
        );
    }
    out.push('\n');
}

fn render_index(out: &mut String, index: &IndexProps, ctx: &mut Ctx) {
    let _ = writeln!(out, "# {}\n", index.title);
    emit_diagram(out, &index.context_diagram, "context", ctx);
    if !index.cards.is_empty() {
        out.push_str("## Modules\n\n");
        for card in &index.cards {
            let _ = writeln!(
                out,
                "- [{}]({}) — {}",
                card.name,
                md_href(&card.href),
                card.meta
            );
        }
        out.push('\n');
    }
}

fn render_module(out: &mut String, module: &ModuleProps, ctx: &mut Ctx) {
    let _ = writeln!(out, "# {}\n", module.name);
    for section in &module.sections {
        render_section(out, section, ctx);
    }
}

/// An authored `[[doc.sidebar]]` page. Its body is Markdown rendered to HTML
/// upstream; inline that HTML (valid inside Markdown) so the page still renders.
fn render_doc(out: &mut String, doc: &DocPageProps) {
    let _ = writeln!(out, "# {}\n", doc.title);
    out.push_str(&doc.html);
    out.push('\n');
}

fn render_section(out: &mut String, section: &NodeSection, ctx: &mut Ctx) {
    let _ = writeln!(out, "## {}\n", section.name);
    let _ = writeln!(
        out,
        "`{} {}` · `{}`\n",
        section.visibility, section.kind, section.fqn
    );

    if !section.tags.is_empty() {
        let tags = section
            .tags
            .iter()
            .map(|tag| format!("`#{tag}`"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(out, "{tags}\n");
    }
    if let Some(summary) = &section.summary {
        let _ = writeln!(out, "{summary}\n");
    }
    if let Some(extended) = &section.extended {
        let _ = writeln!(out, "{extended}\n");
    }

    render_relationships(out, &section.relationships);
    render_scenarios(out, &section.scenarios, &section.id, ctx);
    for (i, diagram) in section.diagrams.iter().enumerate() {
        emit_diagram(out, diagram, &format!("{}-{i}", section.id), ctx);
    }
}

fn render_relationships(out: &mut String, groups: &[RelGroup]) {
    if groups.is_empty() {
        return;
    }
    out.push_str("**Relationships**\n\n");
    for group in groups {
        let _ = writeln!(out, "- _{}_", group.title);
        for item in &group.items {
            let target = match &item.href {
                Some(href) => format!("[{}]({})", item.fqn, md_href(href)),
                None => format!("`{}`", item.fqn),
            };
            let label = item
                .label
                .as_deref()
                .map_or_else(String::new, |l| format!(" — {l}"));
            let _ = writeln!(out, "  - {} {target}{label}", item.edge_kind);
        }
    }
    out.push('\n');
}

fn render_scenarios(out: &mut String, scenarios: &[ScenarioCard], section_id: &str, ctx: &mut Ctx) {
    if scenarios.is_empty() {
        return;
    }
    out.push_str("**Scenarios**\n\n");
    for scenario in scenarios {
        let _ = writeln!(out, "- **{}**", scenario.name);
        for step in &scenario.steps {
            let _ = writeln!(out, "  - _{}_ {}", step.keyword, step.text);
        }
    }
    out.push('\n');
    for (i, scenario) in scenarios.iter().enumerate() {
        emit_diagram(out, &scenario.flow, &format!("{section_id}-flow-{i}"), ctx);
    }
}

/// Writes a diagram as a standalone `diagrams/<stem>.svg` file and references it
/// from the page with a captioned image link. Diagrams arrive pre-rendered in
/// the Markdown site's standalone theme; an adaptive render additionally embeds
/// the `prefers-color-scheme` style block so the file follows the OS scheme on
/// its own. An unprojectable view becomes a one-line note (no file).
fn emit_diagram(out: &mut String, diagram: &Diagram, stem: &str, ctx: &mut Ctx) {
    let (caption, svg) = match diagram {
        Diagram::Svg { caption, svg, .. } => (caption, svg.clone()),
        Diagram::Empty { caption, eyebrow } => {
            let _ = writeln!(out, "_{caption}: no {eyebrow}._\n");
            return;
        }
    };
    let svg = if ctx.theme == Theme::Adaptive {
        embed_adaptive_style(&svg)
    } else {
        svg
    };
    let file = format!("diagrams/{stem}.svg");
    let _ = writeln!(out, "**{caption}**\n");
    let _ = writeln!(out, "![{caption}]({}{file})\n", ctx.prefix);
    ctx.svgs.push(SiteFile::new(file, svg));
}

/// Splices the adaptive style block just after the opening `<svg ...>` tag.
fn embed_adaptive_style(svg: &str) -> String {
    svg.find('>').map_or_else(
        || svg.to_owned(),
        |end| {
            let mut out = String::with_capacity(svg.len() + 512);
            out.push_str(&svg[..=end]);
            out.push_str(&adaptive_style_block());
            out.push_str(&svg[end + 1..]);
            out
        },
    )
}
