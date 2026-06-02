//! Integration tests for the Markdown doc renderer
//! ([`render_markdown_site`](pseudoscript_doc::render_markdown_site)): one `.md`
//! per module plus `index.md`, with every diagram inlined as self-contained SVG.

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
fn writes_markdown_files_only() {
    let site = render();
    assert!(site.file("index.md").is_some(), "index.md should exist");
    assert!(
        site.file("module/shop.md").is_some(),
        "the module page should exist as .md"
    );
    assert!(
        site.files.iter().all(|f| std::path::Path::new(&f.path)
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("html"))),
        "the Markdown renderer must not emit any .html files"
    );
}

#[test]
fn index_inlines_the_context_diagram_and_links_modules() {
    let site = render();
    let index = &site.file("index.md").expect("index.md").contents;
    assert!(index.contains("# Shop"), "site title heading");
    assert!(
        index.contains("<svg"),
        "the context diagram is inlined as SVG"
    );
    assert!(
        index.contains("](module/shop.md)"),
        "module links point at the .md page, got:\n{index}"
    );
}

#[test]
fn module_page_has_node_sections_and_inline_svg() {
    let site = render();
    let page = &site
        .file("module/shop.md")
        .expect("module/shop.md")
        .contents;
    assert!(page.contains("## Shop"), "node section heading");
    assert!(
        page.contains("`public system` · `shop::Shop`"),
        "declaration line, got:\n{page}"
    );
    assert!(page.contains("<svg"), "diagrams are inlined as SVG");
    assert!(
        !page.contains(".html)"),
        "cross-links must target .md, not .html"
    );
}
