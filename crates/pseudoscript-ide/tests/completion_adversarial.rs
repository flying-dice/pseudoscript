//! Adversarial coverage for the IDE's autocomplete: only valid, sensible
//! suggestions should survive. Each case asserts what MUST appear and — the
//! adversarial half — what MUST NOT. Driven through `IdeSession`, the real app
//! path (mount → completion), so a leak here is a leak the editor would show.
//!
//! Caret is marked `~` in the source (a char never valid in `.pds`); the helper
//! strips it and completes at that byte offset.

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

/// Completion labels at the `~` caret in module `m`, with optional externals.
fn labels_ext(src_with_caret: &str, ext: &[(&str, &str)]) -> Vec<String> {
    let offset = u32::try_from(src_with_caret.find('~').expect("caret `~`")).unwrap();
    let src = src_with_caret.replacen('~', "", 1);
    let mut session = IdeSession::new();
    session.mount(mods(&[("m", &src)]), mods(ext));
    session
        .completion("m", offset)
        .into_iter()
        .map(|c| c.label)
        .collect()
}

fn labels(src_with_caret: &str) -> Vec<String> {
    labels_ext(src_with_caret, &[])
}

fn has(labels: &[String], label: &str) -> bool {
    labels.iter().any(|l| l == label)
}

/// Assert every banned label is absent — the adversarial guard.
fn assert_none(labels: &[String], banned: &[&str]) {
    for b in banned {
        assert!(!has(labels, b), "leaked `{b}` into {labels:?}");
    }
}

// --- prose: never complete inside a string or a doc comment ----------------

#[test]
fn no_completion_inside_a_feature_step_string() {
    // Typing prose in `given "…"` must offer nothing — not the keyword set.
    let l =
        labels("//! m\npublic system A;\nfeature F for m::A {\n  given \"a verified ~owner\"\n}\n");
    assert!(l.is_empty(), "completion fired inside a string: {l:?}");
}

#[test]
fn no_completion_inside_a_macro_value_string() {
    let l = labels("//! m\npublic system A {\n  #[schedule = \"0 3 ~* * *\"]\n  job(): void;\n}\n");
    assert!(l.is_empty(), "completion fired inside a macro value: {l:?}");
}

#[test]
fn no_completion_inside_a_doc_comment() {
    let l = labels("//! m\n/// describe the ~system here\npublic system A;\n");
    assert!(l.is_empty(), "completion fired inside a doc comment: {l:?}");
}

#[test]
fn no_completion_inside_the_inner_doc() {
    let l = labels("//! the ~module\npublic system A;\n");
    assert!(l.is_empty(), "completion fired inside the inner doc: {l:?}");
}

// --- type position: only types -------------------------------------------

#[test]
fn type_position_offers_types_only() {
    let l = labels("//! m\npublic data Money { amount: number }\npublic data D { x: ~ }\n");
    assert!(has(&l, "number"), "{l:?}");
    assert!(has(&l, "Result"), "{l:?}");
    assert!(has(&l, "Option"), "{l:?}");
    assert!(has(&l, "Money"), "{l:?}");
    // not keywords, not value markers, not macros
    assert_none(
        &l,
        &[
            "public",
            "container",
            "feature",
            "for",
            "given",
            "Ok",
            "Err",
            "Some",
            "None",
        ],
    );
}

#[test]
fn generic_argument_position_offers_types_only() {
    let l = labels(
        "//! m\npublic data Money { amount: number }\npublic system A {\n  find(): Result<~ > { return find() }\n}\n",
    );
    assert!(has(&l, "Money") || has(&l, "number"), "{l:?}");
    assert_none(&l, &["public", "container", "feature", "Ok", "Err"]);
}

// --- `for` parent / feature target: only nodes ---------------------------

#[test]
fn for_parent_offers_nodes_not_data_or_primitives() {
    let l = labels(
        "//! m\npublic data Rec { x: number }\npublic system Sys;\npublic container C for ~\n",
    );
    assert!(has(&l, "Sys"), "{l:?}");
    // a `data` type and primitives are not nodes — never a parent
    assert_none(
        &l,
        &[
            "Rec",
            "number",
            "string",
            "Result",
            "Option",
            "public",
            "container",
        ],
    );
}

#[test]
fn feature_target_offers_nodes_only() {
    let l = labels("//! m\npublic data Rec;\npublic system Sys;\nfeature F for ~\n");
    assert!(has(&l, "Sys"), "{l:?}");
    assert_none(&l, &["Rec", "number", "given", "when", "then", "feature"]);
}

#[test]
fn container_for_offers_only_systems() {
    // A container's parent must be a system (§4) — never another container,
    // component, person, or data.
    let l = labels(
        "//! m\npublic system Sys;\npublic container Box for m::Sys;\npublic component Comp for m::Box;\npublic person P;\npublic data Rec;\npublic container New for ~\n",
    );
    assert!(has(&l, "Sys"), "valid system parent missing: {l:?}");
    assert_none(&l, &["Box", "Comp", "P", "Rec"]);
}

#[test]
fn component_for_offers_only_containers() {
    // A component's parent must be a container — never a system.
    let l = labels(
        "//! m\npublic system Sys;\npublic container Box for m::Sys;\npublic component New for ~\n",
    );
    assert!(has(&l, "Box"), "valid container parent missing: {l:?}");
    assert_none(&l, &["Sys"]);
}

#[test]
fn binding_type_annotation_offers_types_only() {
    let l = labels(
        "//! m\npublic data Money { amount: number }\npublic system A {\n  go(): void {\n    x: ~\n  }\n}\n",
    );
    assert!(has(&l, "number") || has(&l, "Money"), "{l:?}");
    assert_none(&l, &["container", "public", "given", "Ok", "Err"]);
}

// --- member `.` : only the receiver's members ----------------------------

#[test]
fn cross_module_member_offers_public_only() {
    // `dep::core::Svc.` — a private member of a dependency node is not callable
    // across modules (§8.2), so it must not be offered.
    let ext = &[(
        "dep::core",
        "//! dep::core\npublic system Svc {\n  public open(): void;\n  hidden(): void;\n}\n",
    )];
    let l = labels_ext(
        "//! m\npublic container C for dep::core::Svc {\n  go(): void {\n    dep::core::Svc.~\n  }\n}\n",
        ext,
    );
    assert!(has(&l, "open"), "public member missing: {l:?}");
    assert_none(&l, &["hidden"]);
}

#[test]
fn member_on_unresolved_receiver_is_empty() {
    let l = labels("//! m\npublic system A {\n  go(): void {\n    Nonexistent.~\n  }\n}\n");
    assert!(
        l.is_empty(),
        "members offered for an unresolved receiver: {l:?}"
    );
}

// --- `::` path : public-only across modules ------------------------------

#[test]
fn cross_module_path_offers_public_not_private() {
    let ext = &[(
        "dep::core",
        "//! dep::core\npublic data Pub { x: number }\ndata Priv { y: number }\n",
    )];
    let l = labels_ext("//! m\npublic data D { a: dep::core::~ }\n", ext);
    assert!(has(&l, "Pub"), "public dep symbol missing: {l:?}");
    assert_none(&l, &["Priv"]); // a private dependency symbol must never be offered
}

#[test]
fn dependency_root_offers_only_real_submodules() {
    let ext = &[("dep::core", "//! dep::core\npublic system Sys;\n")];
    let l = labels_ext("//! m\npublic container C for dep::~\n", ext);
    assert!(has(&l, "core"), "{l:?}");
    // not a garbage path segment, not the leaf symbol at this level
    assert_none(&l, &["Sys", "pds_modules", "Priv"]);
}

#[test]
fn colons_after_a_node_offer_nothing_garbage() {
    // A node is not a namespace; `Sys::` resolves to no module.
    let l = labels("//! m\npublic system Sys;\npublic container C for Sys::~\n");
    assert_none(&l, &["Sys", "number", "public"]);
}

// --- `#[` : only macros ---------------------------------------------------

#[test]
fn macro_position_offers_macros_only() {
    let l = labels("//! m\npublic system A {\n  #[~\n  go(): void;\n}\n");
    assert!(
        has(&l, "onevent") || has(&l, "manual") || has(&l, "schedule") || has(&l, "http"),
        "{l:?}"
    );
    assert_none(&l, &["container", "number", "Result", "public"]);
}

// --- hygiene --------------------------------------------------------------

#[test]
fn no_duplicate_labels() {
    let l = labels("//! m\npublic system Sys;\npublic container C for ~\n");
    let mut seen = std::collections::HashSet::new();
    for label in &l {
        assert!(seen.insert(label), "duplicate label `{label}` in {l:?}");
    }
}
