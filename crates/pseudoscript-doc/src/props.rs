//! The serialisable page model handed to the Svelte renderer.
//!
//! Every derived value the page needs — resolved hrefs, anchor ids, edge
//! labels, "N items" counts — is precomputed in Rust so the Svelte components
//! do no graph logic. All user text is carried **raw**: Svelte auto-escapes
//! `{…}` interpolation, so escaping here would double-encode. Diagrams carry the
//! emit [`C4Scene`]/[`SequenceScene`] geometry, not pre-rendered SVG; the client
//! islands ([`crate`] `web/lib/C4Flow.svelte`, `FlowTimeline.svelte`) lay them
//! out and animate them.

use pseudoscript_emit::{C4Scene, SequenceScene};
use pseudoscript_layout::sequence::Layout;
use serde::{Deserialize, Serialize};

/// The complete props for one page. Serialised to JSON, passed to the SSR
/// `renderPage` and embedded verbatim as `window.__DATA__` for hydration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageProps {
    /// Site-wide presentation, identical across pages.
    pub site: SiteInfo,
    /// Authored doc-page groups, shown above the module tree (`[[doc.sidebar]]`).
    /// Empty when no docs are configured. Hrefs prefixed for this page's depth.
    pub doc_groups: Vec<SidebarDocGroup>,
    /// The sidebar tree, with hrefs already prefixed for this page's depth.
    pub sidebar: Vec<SidebarModule>,
    /// The page body: the index, one module's sections, or an authored doc page.
    pub page: PageBody,
}

/// Site-wide presentation derived from [`crate::DocConfig`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SiteInfo {
    /// The site title.
    pub name: String,
    /// `"light"` or `"dark"`.
    pub theme: String,
    /// The logo's filename at the site root, when configured.
    pub logo_filename: Option<String>,
    /// The `../` prefix to the site root for this page's depth (`""` for the
    /// index, `"../"` for module and doc pages), applied to the brand link and
    /// logo.
    pub prefix: String,
}

/// One `[[doc.sidebar]]` group in the sidebar: a heading and its page links.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidebarDocGroup {
    /// The group heading.
    pub title: String,
    /// The group's page links.
    pub items: Vec<SidebarDocItem>,
}

/// One authored doc-page link in the sidebar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidebarDocItem {
    /// The page title (the link text).
    pub title: String,
    /// The href to the page (already prefixed for this page's depth).
    pub href: String,
}

/// One sidebar module entry: a module page link plus its node tree.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidebarModule {
    /// The module FQN, shown as the label and used as the `data-search` value.
    pub label: String,
    /// The href to the module page (already prefixed for this page's depth).
    pub href: String,
    /// The recursive node tree under this module.
    pub nodes: Vec<SidebarNode>,
}

/// One node in the sidebar tree, recursing into its same-module children.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidebarNode {
    /// The node's simple name (the label).
    pub name: String,
    /// The node FQN, used as the `data-search` value.
    pub fqn: String,
    /// The node kind keyword, driving the kind-dot colour.
    pub kind: String,
    /// The href to the node's section (already prefixed for this page's depth).
    pub href: String,
    /// Same-module, non-callable children.
    pub children: Vec<SidebarNode>,
}

/// The page body: index, module, or authored doc page.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum PageBody {
    /// The landing page.
    Index(IndexProps),
    /// One module's page.
    Module(ModuleProps),
    /// One authored Markdown doc page (`[[doc.sidebar]]`).
    Doc(DocPageProps),
}

/// An authored doc page's content: its title and rendered Markdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocPageProps {
    /// The page title, shown as the page heading.
    pub title: String,
    /// The page body as HTML, rendered from Markdown. Mounted with `{@html}`.
    pub html: String,
}

/// The index page content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexProps {
    /// The site title, shown in the page header.
    pub title: String,
    /// The system-context diagram.
    pub context_diagram: Diagram,
    /// One card per module.
    pub cards: Vec<ModuleCard>,
}

/// A module card on the index page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModuleCard {
    /// The module FQN.
    pub name: String,
    /// The href to the module page (already prefixed).
    pub href: String,
    /// The "N item(s)" line.
    pub meta: String,
}

/// A module page's content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModuleProps {
    /// The module FQN, shown as the page header.
    pub name: String,
    /// One section per node declared in the module, sorted by FQN.
    pub sections: Vec<NodeSection>,
}

/// One node's documentation section.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeSection {
    /// The in-page anchor id.
    pub id: String,
    /// The node kind keyword (drives the kind badge).
    pub kind: String,
    /// The node's simple name.
    pub name: String,
    /// The node FQN, shown as a code line.
    pub fqn: String,
    /// `"public"` or `"private"`.
    pub visibility: String,
    /// The `///` summary, when present.
    pub summary: Option<String>,
    /// The extended description, when present.
    pub extended: Option<String>,
    /// The `#tags`, in source order.
    pub tags: Vec<String>,
    /// The parent / inbound / outbound relationship groups.
    pub relationships: Vec<RelGroup>,
    /// The BDD scenario cards targeting this node.
    pub scenarios: Vec<ScenarioCard>,
    /// The embedded diagrams (container/component + per-trigger sequence).
    pub diagrams: Vec<Diagram>,
}

/// One relationship group ("Parent", "Inbound", or "Outbound").
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelGroup {
    /// The group heading.
    pub title: String,
    /// The endpoints in the group.
    pub items: Vec<RelItem>,
}

/// One relationship endpoint.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelItem {
    /// The edge-kind word ("for"/"in"/"call"/"trigger"/"from").
    pub edge_kind: String,
    /// Whether to show the outbound arrow.
    pub arrow: bool,
    /// The endpoint FQN text.
    pub fqn: String,
    /// The resolved href, or `None` to render plain text.
    pub href: Option<String>,
    /// The trailing edge label (e.g. the call method), when present.
    pub label: Option<String>,
}

/// One BDD scenario card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioCard {
    /// The scenario name.
    pub name: String,
    /// The scenario `///` summary, when present.
    pub summary: Option<String>,
    /// The scenario extended description, when present.
    pub extended: Option<String>,
    /// The scenario `#tags`.
    pub tags: Vec<String>,
    /// The ordered given/when/then steps.
    pub steps: Vec<Step>,
    /// The feature's flow diagram (`LANG.md` §9.5).
    pub flow: Diagram,
}

/// One scenario step.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Step {
    /// The step keyword (given/when/then/and/but).
    pub keyword: String,
    /// The step prose.
    pub text: String,
}

/// A diagram figure. C4 and sequence carry the emit scene geometry for a client
/// island to draw; `svg` carries a pre-rendered document; `empty` marks a view
/// that failed to project.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "diagram", rename_all = "camelCase")]
pub(crate) enum Diagram {
    /// A C4 view, laid out client-side by Svelte Flow.
    C4 {
        /// The figure caption.
        caption: String,
        /// The laid-out C4 scene (nodes + edges).
        scene: C4Scene,
    },
    /// A sequence view, drawn as the IDE's lifeline/message diagram.
    Sequence {
        /// The figure caption.
        caption: String,
        /// The sequence scene (participants + ordered items).
        scene: SequenceScene,
        /// The positioned layout (lifelines, activations, fragments, messages)
        /// from `pseudoscript-layout` — the same geometry the web IDE renders,
        /// so the figure needs no client-side layout engine.
        layout: Layout,
    },
    /// A data entity or feature flow view (`LANG.md` §9.4, §9.5), pre-rendered
    /// to SVG in the site's configured theme — no client island needed.
    Svg {
        /// The figure caption.
        caption: String,
        /// The self-contained SVG document.
        svg: String,
    },
    /// A view that could not be projected (e.g. an empty boundary).
    Empty {
        /// The figure caption.
        caption: String,
        /// The diagram-kind word for the placeholder text.
        eyebrow: String,
    },
}

/// The SSR result an [`SsrEngine`](crate::SsrEngine) returns: the
/// `<svelte:head>` contents and the rendered body markup.
#[derive(Debug, Clone, Deserialize)]
pub struct RenderedPage {
    /// Head tags emitted by the component (may be empty).
    pub head: String,
    /// The rendered body markup, dropped into the page shell.
    pub body: String,
}
