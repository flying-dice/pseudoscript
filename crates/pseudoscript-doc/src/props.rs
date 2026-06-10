//! The serialisable page model handed to the Svelte renderer.
//!
//! Every derived value the page needs — resolved hrefs, anchor ids, edge
//! labels, "N items" counts — is precomputed in Rust so the Svelte components
//! do no graph logic. All user text is carried **raw**: Svelte auto-escapes
//! `{…}` interpolation, so escaping here would double-encode. Every diagram is
//! pre-rendered, deterministic SVG (the same output as `pds svg`, under the
//! adaptive palette); the client adds pan/zoom only. Pages are pure SSR — no
//! hydration — so props are embedded in the document only on the universe page,
//! whose 3D island reads them.

use pseudoscript_universe::FlowDef;
use serde::{Deserialize, Serialize};

/// The complete props for one page. Serialised to JSON and passed to the SSR
/// `renderPage`; embedded as `window.__DATA__` only on the universe page.
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
    /// The header navigation links (Overview, Universe, Health with its
    /// finding-count badge), hrefs prefixed for this page's depth.
    pub nav: Vec<NavLink>,
    /// The breadcrumb trail; the trailing crumb (this page) has no href.
    pub crumbs: Vec<Crumb>,
    /// The page body.
    pub page: PageBody,
}

/// One header navigation link.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavLink {
    /// The link text.
    pub label: String,
    /// The href, prefixed for this page's depth.
    pub href: String,
    /// A count badge (the Health link's finding count); `None` elsewhere.
    pub badge: Option<String>,
    /// Whether this link is the current page.
    pub current: bool,
}

/// One breadcrumb hop.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Crumb {
    /// The crumb text.
    pub label: String,
    /// The href, prefixed for this page's depth; `None` on the trailing crumb.
    pub href: Option<String>,
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

/// The page body: index, module, authored doc page, the 3D universe, or the
/// architecture-health report.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum PageBody {
    /// The landing page.
    Index(IndexProps),
    /// One module's page.
    Module(ModuleProps),
    /// One authored Markdown doc page (`[[doc.sidebar]]`).
    Doc(DocPageProps),
    /// The 3D universe page — the only page whose props are embedded in the
    /// document (its island reads them).
    Universe(UniverseProps),
    /// The architecture-health report.
    Health(HealthProps),
}

/// The universe page's content: everything the 3D island draws.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UniverseProps {
    /// The placed structural nodes (id, level word, parent id or `null`).
    pub nodes: Vec<UniverseNode>,
    /// The directed relationship edges with their traffic.
    pub edges: Vec<UniverseEdge>,
    /// The entry-point flows, traced and coloured in Rust.
    pub flows: Vec<FlowDef>,
    /// Each placed node's documentation href (prefixed for this page's depth),
    /// so clicking a sphere can navigate to its docs.
    pub hrefs: Vec<NodeHref>,
}

/// One placed universe node, mirroring `pseudoscript_universe::NodeOut` with an
/// optional parent (empty string → `None` for the renderer).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UniverseNode {
    /// The node's stable model FQN.
    pub id: String,
    /// The C4 level word (`system`/`container`/`component`/`person`).
    pub level: String,
    /// The containment parent's id, when placed.
    pub parent: Option<String>,
}

/// One universe relationship edge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UniverseEdge {
    /// Source node id.
    pub from: String,
    /// Target node id.
    pub to: String,
    /// The lifted call count.
    pub traffic: u32,
}

/// A universe node's documentation link.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeHref {
    /// The node id.
    pub id: String,
    /// The href to its documentation section (prefixed for the page's depth).
    pub href: String,
}

/// The health page's content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthProps {
    /// Every finding, attributed and sorted by severity, module, then line.
    pub entries: Vec<HealthEntry>,
    /// How many entries are errors.
    pub error_count: usize,
    /// How many entries are warnings.
    pub warning_count: usize,
}

/// One architecture-health finding, attributed to the node whose section it
/// belongs on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthEntry {
    /// The declaring module FQN.
    pub module: String,
    /// `"error"` or `"warning"`.
    pub severity: String,
    /// The stable rule code (e.g. `PDS-ARCH-001`), when the diagnostic has one.
    pub code: Option<String>,
    /// The rule's principle-article URL, when published.
    pub code_url: Option<String>,
    /// The finding message.
    pub message: String,
    /// 1-based source line.
    pub line: u32,
    /// 1-based source column.
    pub column: u32,
    /// The owning node's FQN (empty when no node encloses the span).
    pub node_fqn: String,
    /// The href to the owning node's section (the module page when no node
    /// encloses the span), prefixed for the page's depth.
    pub href: String,
}

/// One inline finding badge on a node's section.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionDiagnostic {
    /// `"error"` or `"warning"`.
    pub severity: String,
    /// The stable rule code, when the diagnostic has one.
    pub code: Option<String>,
    /// The rule's principle-article URL, when published.
    pub code_url: Option<String>,
    /// The finding message.
    pub message: String,
    /// 1-based source line.
    pub line: u32,
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
    /// The stats strip summarising the model and its findings.
    pub stats: SiteStats,
}

/// The index page's stats strip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SiteStats {
    /// How many systems the model declares.
    pub systems: usize,
    /// How many containers.
    pub containers: usize,
    /// How many components.
    pub components: usize,
    /// How many entry-point flows.
    pub flows: usize,
    /// How many health findings.
    pub findings: usize,
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
    /// The health findings attributed to this node, shown as inline badges.
    pub diagnostics: Vec<SectionDiagnostic>,
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

/// A diagram figure. Every view ships as deterministic server-rendered SVG —
/// the same output as `pds svg`, under the adaptive palette so the figure
/// follows the active scheme; `empty` marks a view that failed to project.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "diagram", rename_all = "camelCase")]
pub(crate) enum Diagram {
    /// A pre-rendered figure.
    Svg {
        /// The figure caption.
        caption: String,
        /// The small label above the caption.
        eyebrow: String,
        /// The view-kind word (`c4`/`sequence`/`entity`/`flow`), exposed as the
        /// figure's `data-diagram` hook.
        kind: String,
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
