//! Adversarial coverage for hover: it resolves a real symbol and stays silent
//! everywhere else (whitespace, punctuation, keywords, prose). Driven through
//! `IdeSession` — the real app path. Caret marked `~`.

use pseudoscript_ide::{IdeSession, Module};

/// `[(fqn, source)]` → the typed module list `mount` takes.
fn mods(list: &[(&str, &str)]) -> Vec<Module> {
    list.iter()
        .map(|(fqn, source)| Module {
            fqn: (*fqn).to_owned(),
            source: (*source).to_owned(),
        })
        .collect()
}

/// The hover Markdown at the `~` caret, or `None` when hover is empty.
fn hover(src_with_caret: &str) -> Option<String> {
    let offset = u32::try_from(src_with_caret.find('~').expect("caret `~`")).unwrap();
    let src = src_with_caret.replacen('~', "", 1);
    let mut session = IdeSession::new();
    session.mount(mods(&[("m", &src)]), Vec::new());
    session.hover("m", offset).map(|h| h.contents.value)
}

#[test]
fn hover_resolves_a_node_reference() {
    let md = hover("//! m\npublic system Banking;\npublic container Web for m::Bank~ing;\n")
        .expect("a node reference hovers");
    assert!(md.contains("m::Banking"), "{md}");
}

#[test]
fn hover_on_whitespace_is_silent() {
    assert!(hover("//! m\npublic system A;\n~\n").is_none());
}

#[test]
fn hover_on_a_keyword_is_silent() {
    // The caret on the `public` keyword resolves to nothing.
    assert!(hover("//! m\npub~lic system A;\n").is_none());
}

#[test]
fn hover_inside_a_string_is_silent() {
    assert!(
        hover("//! m\npublic system A;\nfeature F for m::A {\n  given \"some te~xt\"\n}\n")
            .is_none()
    );
}

#[test]
fn hover_on_punctuation_is_silent() {
    assert!(hover("//! m\npublic system A;~\n").is_none());
}
