//! Documentation-site generation harness for `pseudoscript-doc`.
//!
//! A hand-written `banking::core` model exercises every documented element — a
//! person, a system with a container (via `for`), a container with a component,
//! a `data`, and a `#[manual]` triggered callable with a body, all carrying
//! `///` summaries and a `#tag`. The scenarios assert `render_site` produces the
//! index, module pages, embedded diagrams, cross-links, shared assets, and
//! deterministic output (`LANG.md` §9.3).

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
    n = Ledger.Append(posting)
    return n
  }
}
"#;

#[derive(Debug, Default, World)]
struct DocWorld {
    graph: Option<Graph>,
    site: Option<Site>,
    again: Option<Site>,
}

#[given("the banking workspace model")]
fn given_model(world: &mut DocWorld) {
    world.graph = Some(graph(&[WorkspaceModule::new("banking::core", MODEL)]));
}

#[allow(clippy::needless_pass_by_value)]
#[when(regex = r#"^I render the site titled "([^"]*)"$"#)]
fn render(world: &mut DocWorld, title: String) {
    let config = DocConfig {
        name: title,
        ..DocConfig::default()
    };
    world.site = Some(render_site(world.graph(), &config));
}

#[allow(clippy::needless_pass_by_value)]
#[when(regex = r#"^I render the site titled "([^"]*)" again$"#)]
fn render_again(world: &mut DocWorld, title: String) {
    let config = DocConfig {
        name: title,
        ..DocConfig::default()
    };
    world.again = Some(render_site(world.graph(), &config));
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
#[then(regex = r#"^the file "([^"]*)" contains an inline SVG$"#)]
fn file_contains_svg(world: &mut DocWorld, path: String) {
    assert!(
        world.contents(&path).contains("<svg"),
        "{path:?} contains an inline <svg"
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
#[then(regex = r#"^the section "([^"]*)" on "([^"]*)" contains an inline SVG$"#)]
fn section_contains_svg(world: &mut DocWorld, fqn: String, path: String) {
    let contents = world.contents(&path);
    let id = anchor(&fqn);
    let start = contents
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("section {fqn:?} present in {path:?}"));
    // the section runs to the next `<section class="node"` or end of doc
    let rest = &contents[start..];
    let end = rest[1..]
        .find("<section class=\"node\"")
        .map_or(rest.len(), |i| i + 1);
    assert!(
        rest[..end].contains("<svg"),
        "section {fqn:?} embeds an inline <svg"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the file "([^"]*)" links to the anchor of "([^"]*)"$"#)]
fn links_to_anchor(world: &mut DocWorld, path: String, fqn: String) {
    let contents = world.contents(&path);
    let needle = format!("href=\"banking.core.html#{}\"", anchor(&fqn));
    let plain = format!("#{}\"", anchor(&fqn));
    assert!(
        contents.contains(&needle) || contents.contains(&plain),
        "{path:?} links the anchor of {fqn:?}"
    );
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

    fn paths(&self) -> Vec<&str> {
        self.site().files.iter().map(|f| f.path.as_str()).collect()
    }
}

fn main() {
    futures::executor::block_on(DocWorld::run(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
