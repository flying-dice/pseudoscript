//! Markdown documentation renderer: each page as a self-contained `.md` file
//! with its diagrams inlined as SVG.
//!
//! An engine-free alternative to the Svelte HTML site — the same page model
//! ([`build_pages`](crate::render::build_pages)) rendered to Markdown instead of
//! driven through SSR, so it needs no JavaScript and runs on every target. The
//! output reads on GitHub, in an editor preview, or by an agent.
//!
//! Diagrams are embedded as self-contained SVG rendered with the **light**
//! palette (dark ink on white cards), which stays legible on any Markdown
//! background — the dark palette's light ink would vanish on a white page.

use std::fmt::Write as _;

use pseudoscript_emit::{Scene, Theme, render_svg_themed};

use crate::props::{
    Diagram, DocPageProps, IndexProps, ModuleProps, NodeSection, PageBody, PageProps, RelGroup,
    ScenarioCard,
};
use crate::site::SiteFile;

/// Renders every built page to a Markdown [`SiteFile`], mirroring the HTML
/// site's paths with a `.md` extension.
pub(crate) fn pages_to_markdown(pages: &[(String, PageProps)]) -> Vec<SiteFile> {
    pages
        .iter()
        .map(|(path, props)| SiteFile::new(md_path(path), render_page(props)))
        .collect()
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

/// Renders one page's body to Markdown.
fn render_page(props: &PageProps) -> String {
    let mut out = String::new();
    match &props.page {
        PageBody::Index(index) => render_index(&mut out, index),
        PageBody::Module(module) => render_module(&mut out, module),
        PageBody::Doc(doc) => render_doc(&mut out, doc),
    }
    out
}

fn render_index(out: &mut String, index: &IndexProps) {
    let _ = writeln!(out, "# {}\n", index.title);
    render_diagram(out, &index.context_diagram);
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

fn render_module(out: &mut String, module: &ModuleProps) {
    let _ = writeln!(out, "# {}\n", module.name);
    for section in &module.sections {
        render_section(out, section);
    }
}

/// An authored `[[doc.sidebar]]` page. Its body is Markdown rendered to HTML
/// upstream; inline that HTML (valid inside Markdown) so the page still renders.
fn render_doc(out: &mut String, doc: &DocPageProps) {
    let _ = writeln!(out, "# {}\n", doc.title);
    out.push_str(&doc.html);
    out.push('\n');
}

fn render_section(out: &mut String, section: &NodeSection) {
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
    render_scenarios(out, &section.scenarios);
    for diagram in &section.diagrams {
        render_diagram(out, diagram);
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

fn render_scenarios(out: &mut String, scenarios: &[ScenarioCard]) {
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
}

/// Inlines a diagram as a self-contained SVG, captioned. An unprojectable view
/// becomes a one-line note.
fn render_diagram(out: &mut String, diagram: &Diagram) {
    match diagram {
        Diagram::C4 { caption, scene } => {
            figure(
                out,
                caption,
                &render_svg_themed(&Scene::C4(scene.clone()), Theme::Light),
            );
        }
        Diagram::Sequence { caption, scene, .. } => {
            figure(
                out,
                caption,
                &render_svg_themed(&Scene::Sequence(scene.clone()), Theme::Light),
            );
        }
        Diagram::Empty { caption, eyebrow } => {
            let _ = writeln!(out, "_{caption}: no {eyebrow}._\n");
        }
    }
}

/// A captioned SVG figure. The SVG is set off by blank lines, since Markdown
/// treats an HTML block as raw only when surrounded by them.
fn figure(out: &mut String, caption: &str, svg: &str) {
    let _ = writeln!(out, "**{caption}**\n");
    out.push_str(svg);
    out.push_str("\n\n");
}
