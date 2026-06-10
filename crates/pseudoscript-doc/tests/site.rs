//! Svelte documentation-site generation harness for `pseudoscript-doc`.
//!
//! Mirrors the `pseudoscript-doc` contract on the same `banking::core` model —
//! every documented element, a `feature` scenario, and a triggered callable.
//! Server-rendered text (names, summaries, tags, scenario steps, cross-links)
//! is asserted directly; every diagram is server-rendered inline SVG carried
//! by a `data-diagram` figure, page data is embedded only on the universe
//! page, and the chrome (skip link, theme toggle, classic deferred scripts)
//! ships on every page. Output must stay byte-for-byte deterministic
//! (`LANG.md` §9.3). Requires the prebuilt bundle (`src/assets/*`).

use cucumber::{World, given, then, when};
use pseudoscript_doc::{DocConfig, Site, render_site};
use pseudoscript_model::{Graph, WorkspaceModule, graph};

/// The fixture workspace source: one `banking::core` module covering every kind
/// of documented element.
const MODEL: &str = r#"//! banking::core

/// A retail banking customer.
/// #external
public person Customer;

/// A double-entry posting.
public data Posting { id: number, amount: number }

/// The core banking platform.
/// #core
public system Bank;

/// Records every posting.
/// #critical
public container Ledger for Bank {
  /// Append a posting to the ledger.
  public Append(posting: Posting): number;
}

/// A balanced posting is recorded.
feature AppendPosting for Ledger {
  given "a balanced posting"
  when "it is appended"
  then "the ledger records it"
}

/// Public-facing API surface.
public container Api for Bank {
  /// Validate and forward a request.
  component Validator for Api {
    check(posting: Posting): number;
  }

  /// Post a new entry.
  /// #entrypoint
  #[manual]
  public Post(posting: Posting): number {
    n = number from Ledger.Append(posting)
    return n
  }
}
"#;

/// A pair of modules seeding PDS-ARCH-001: `shop` reaches into `core`'s
/// component instead of the container face.
const LINT_CALLER: &str = "//! shop\n\npublic system Shop;\n\npublic container Front for Shop {\n  #[manual]\n  public Buy(): number {\n    n = number from core::Gate.check()\n    return n\n  }\n}\n";
const LINT_CORE: &str = "//! core\n\npublic system Core;\n\npublic container Api for Core {\n  /// The gate.\n  public component Gate for Api {\n    public check(): number;\n  }\n}\n";

#[derive(Debug, Default, World)]
struct DocWorld {
    graph: Option<Graph>,
    modules: Vec<WorkspaceModule>,
    site: Option<Site>,
    again: Option<Site>,
}

#[given("the banking workspace model")]
fn given_model(world: &mut DocWorld) {
    world.graph = Some(graph(&[WorkspaceModule::new("banking::core", MODEL)]));
}

#[given("a workspace with a facade bypass")]
fn given_lint_model(world: &mut DocWorld) {
    world.modules = vec![
        WorkspaceModule::new("shop", LINT_CALLER),
        WorkspaceModule::new("core", LINT_CORE),
    ];
    world.graph = Some(graph(&world.modules));
}

#[allow(clippy::needless_pass_by_value)]
#[when(regex = r#"^I render the site titled "([^"]*)"$"#)]
fn render(world: &mut DocWorld, title: String) {
    let config = DocConfig {
        name: title,
        ..DocConfig::default()
    };
    world.site = Some(render_site(world.graph(), &config, &[]));
}

#[allow(clippy::needless_pass_by_value)]
#[when(regex = r#"^I render the checked site titled "([^"]*)"$"#)]
fn render_checked(world: &mut DocWorld, title: String) {
    let config = DocConfig {
        name: title,
        ..DocConfig::default()
    };
    let per_module =
        pseudoscript_model::check_workspace_modules_with_externals(&world.modules, &[]);
    let diagnostics = pseudoscript_doc::prepare_diagnostics(&world.modules, &per_module);
    world.site = Some(render_site(world.graph(), &config, &diagnostics));
}

#[allow(clippy::needless_pass_by_value)]
#[when(regex = r#"^I render the site titled "([^"]*)" again$"#)]
fn render_again(world: &mut DocWorld, title: String) {
    let config = DocConfig {
        name: title,
        ..DocConfig::default()
    };
    world.again = Some(render_site(world.graph(), &config, &[]));
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" exists$"#)]
fn file_exists(world: &mut DocWorld, path: String) {
    assert!(
        world.site().file(&path).is_some(),
        "site has file {path:?}; has: {:?}",
        world.paths(),
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" contains "([^"]*)"$"#)]
fn file_contains(world: &mut DocWorld, path: String, needle: String) {
    let contents = world.contents(&path);
    assert!(contents.contains(&needle), "{path:?} contains {needle:?}");
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" does not contain "([^"]*)"$"#)]
fn file_not_contains(world: &mut DocWorld, path: String, needle: String) {
    let contents = world.contents(&path);
    assert!(
        !contents.contains(&needle),
        "{path:?} must not contain {needle:?}"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" embeds an? "([^"]*)" diagram$"#)]
fn file_embeds_diagram(world: &mut DocWorld, path: String, kind: String) {
    let needle = format!("data-diagram=\"{kind}\"");
    assert!(
        world.contents(&path).contains(&needle),
        "{path:?} embeds a {kind:?} diagram figure"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" references "([^"]*)"$"#)]
fn file_references(world: &mut DocWorld, path: String, asset: String) {
    let contents = world.contents(&path);
    assert!(
        contents.contains(&format!("href=\"{asset}\""))
            || contents.contains(&format!("src=\"{asset}\"")),
        "{path:?} links {asset:?}"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the section "([^"]*)" on "([^"]*)" embeds an? "([^"]*)" diagram$"#)]
fn section_embeds_diagram(world: &mut DocWorld, fqn: String, path: String, kind: String) {
    let contents = world.contents(&path);
    let id = anchor(&fqn);
    let start = contents
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("section {fqn:?} present in {path:?}"));
    let rest = &contents[start..];
    let end = rest[1..]
        .find("<section class=\"node\"")
        .map_or(rest.len(), |i| i + 1);
    assert!(
        rest[..end].contains(&format!("data-diagram=\"{kind}\"")),
        "section {fqn:?} embeds a {kind:?} diagram"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" links to the anchor of "([^"]*)"$"#)]
fn links_to_anchor(world: &mut DocWorld, path: String, fqn: String) {
    // The strict sibling-page form only: every section also carries a
    // same-page `href="#<anchor>"` self-link, so a bare-anchor fallback would
    // pass even with cross-links broken.
    let contents = world.contents(&path);
    let needle = format!("href=\"banking.core.html#{}\"", anchor(&fqn));
    assert!(
        contents.contains(&needle),
        "{path:?} links the anchor of {fqn:?}"
    );
}

/// Every `data-diagram` figure carries an inline `<svg` document before its
/// closing `</figure>` — no figure is an empty island canvas.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^every diagram figure on "([^"]*)" contains inline SVG$"#)]
fn figures_carry_inline_svg(world: &mut DocWorld, path: String) {
    let contents = world.contents(&path);
    let mut figures = 0;
    for chunk in contents.split("<figure").skip(1) {
        if !chunk.starts_with(' ') && !chunk.starts_with('>') {
            continue;
        }
        let figure = chunk.split("</figure>").next().unwrap_or(chunk);
        if figure.contains("data-diagram=") {
            figures += 1;
            assert!(
                figure.contains("<svg"),
                "a data-diagram figure in {path:?} lacks inline SVG"
            );
        }
    }
    assert!(figures > 0, "{path:?} has at least one diagram figure");
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^every page contains "([^"]*)"$"#)]
fn every_page_contains(world: &mut DocWorld, needle: String) {
    for (path, contents) in world.pages() {
        assert!(contents.contains(&needle), "{path:?} contains {needle:?}");
    }
}

/// `client.js` loads as a classic deferred script on every page — never a
/// module (Chrome blocks module scripts under `file://`).
#[then("every page loads the client script as a classic deferred script")]
fn every_page_classic_client_script(world: &mut DocWorld) {
    for (path, contents) in world.pages() {
        assert!(
            contents.contains("<script defer src="),
            "{path:?} loads a classic deferred script"
        );
        assert!(
            !contents.contains("type=\"module\""),
            "{path:?} must not load module scripts"
        );
    }
}

#[then("both renders are identical")]
fn renders_identical(world: &mut DocWorld) {
    let first = world.site();
    let second = world.again.as_ref().expect("a second render");
    assert_eq!(first, second, "render_site is deterministic");
}

/// The anchor id for an FQN, mirroring the crate's URL scheme.
fn anchor(fqn: &str) -> String {
    fqn.replace("::", "-")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

impl DocWorld {
    fn graph(&self) -> &Graph {
        self.graph.as_ref().expect("a built graph")
    }

    fn site(&self) -> &Site {
        self.site.as_ref().expect("a rendered site")
    }

    fn contents(&self, path: &str) -> &str {
        &self
            .site()
            .file(path)
            .unwrap_or_else(|| panic!("site has file {path:?}; has: {:?}", self.paths()))
            .contents
    }

    /// Every generated HTML page, as `(path, contents)`.
    fn pages(&self) -> Vec<(&str, &str)> {
        self.site()
            .files
            .iter()
            .filter(|f| {
                std::path::Path::new(&f.path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
            })
            .map(|f| (f.path.as_str(), f.contents.as_str()))
            .collect()
    }

    fn paths(&self) -> Vec<&str> {
        self.site().files.iter().map(|f| f.path.as_str()).collect()
    }
}

fn main() {
    futures::executor::block_on(DocWorld::run(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
