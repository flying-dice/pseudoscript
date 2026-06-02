//! Integration tests for the Markdown doc renderer
//! ([`render_markdown_site`](pseudoscript_doc::render_markdown_site)): one `.md`
//! per module plus `index.md`, with each diagram written as a standalone `.svg`
//! file and referenced (`![caption](…)`) — inline SVG is dropped by many
//! Markdown renderers, a referenced image is not.

use pseudoscript_doc::{DocConfig, Site, render_markdown_site};
use pseudoscript_model::{WorkspaceModule, graph};

const MODEL: &str = "\
//! shop — a tiny worked model.

public person Buyer {
  /// Place an order through checkout.
  #[manual]
  public order(): void {
    Checkout.place()
  }
}

public system Shop;

public container Checkout for Shop {
  /// Place an order.
  #[manual]
  public place(): void { }
}
";

fn render() -> Site {
    let model = graph(&[WorkspaceModule::new("shop", MODEL)]);
    let config = DocConfig {
        name: "Shop".to_owned(),
        ..DocConfig::default()
    };
    render_markdown_site(&model, &config)
}

#[test]
fn writes_markdown_pages_and_svg_assets_no_html() {
    let site = render();
    assert!(site.file("index.md").is_some(), "index.md should exist");
    assert!(
        site.file("module/shop.md").is_some(),
        "the module page should exist as .md"
    );
    assert!(
        site.file("diagrams/context.svg").is_some(),
        "the context diagram should be a standalone .svg asset"
    );
    assert!(
        site.files.iter().all(|f| std::path::Path::new(&f.path)
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("html"))),
        "the Markdown renderer must not emit any .html files"
    );
}

#[test]
fn pages_reference_svg_files_and_never_inline_them() {
    let site = render();

    let index = &site.file("index.md").expect("index.md").contents;
    assert!(index.contains("# Shop"), "site title heading");
    assert!(
        index.contains("![") && index.contains("](diagrams/context.svg)"),
        "index references the context SVG as an image, got:\n{index}"
    );
    assert!(!index.contains("<svg"), "index.md must not inline SVG");
    assert!(
        index.contains("](module/shop.md)"),
        "module links point at the .md page"
    );

    let page = &site
        .file("module/shop.md")
        .expect("module/shop.md")
        .contents;
    assert!(page.contains("## Shop"), "node section heading");
    assert!(
        page.contains("`public system` · `shop::Shop`"),
        "declaration line, got:\n{page}"
    );
    assert!(
        page.contains("](../diagrams/"),
        "a module page references the root diagrams dir via ../, got:\n{page}"
    );
    assert!(!page.contains("<svg"), "module page must not inline SVG");
    assert!(
        !page.contains(".html)"),
        "cross-links target .md, not .html"
    );
}

#[test]
fn svg_assets_carry_real_content_on_a_font_group() {
    let site = render();
    assert!(
        site.files.iter().any(|f| f.path.starts_with("diagrams/")
            && std::path::Path::new(&f.path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))),
        "expected diagrams/*.svg files"
    );

    let context = site
        .file("diagrams/context.svg")
        .expect("diagrams/context.svg");
    assert!(context.contents.starts_with("<svg"));
    assert!(context.contents.trim_end().ends_with("</svg>"));
    // Font on a group, so renderers that don't inherit it from the root <svg>
    // (e.g. JSVG) still draw the labels.
    assert!(
        context.contents.contains("<g font-family="),
        "the diagram font must be set on a group"
    );
}
