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

use pseudoscript_emit::{Scene, Theme, render_svg_themed};

use crate::props::{
    Diagram, DocPageProps, IndexProps, ModuleProps, NodeSection, PageBody, PageProps, RelGroup,
    ScenarioCard,
};
use crate::site::SiteFile;

/// State threaded through one page: the diagram theme, the page's `../`-to-root
/// prefix (so an image link resolves to the site-root `diagrams/` dir), and the
/// collector the standalone `.svg` files are pushed onto.
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
    }
    out
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
/// from the page with a captioned image link. An unprojectable view becomes a
/// one-line note (no file).
fn emit_diagram(out: &mut String, diagram: &Diagram, stem: &str, ctx: &mut Ctx) {
    let (caption, svg) = match diagram {
        Diagram::C4 { caption, scene } => (
            caption,
            render_svg_themed(&Scene::C4(scene.clone()), ctx.theme),
        ),
        Diagram::Sequence { caption, scene, .. } => (
            caption,
            render_svg_themed(&Scene::Sequence(scene.clone()), ctx.theme),
        ),
        // Pre-rendered upstream in the site's configured theme — the same theme
        // this Markdown render uses.
        Diagram::Svg { caption, svg } => (caption, svg.clone()),
        Diagram::Empty { caption, eyebrow } => {
            let _ = writeln!(out, "_{caption}: no {eyebrow}._\n");
            return;
        }
    };
    let file = format!("diagrams/{stem}.svg");
    let _ = writeln!(out, "**{caption}**\n");
    let _ = writeln!(out, "![{caption}]({}{file})\n", ctx.prefix);
    ctx.svgs.push(SiteFile::new(file, svg));
}
