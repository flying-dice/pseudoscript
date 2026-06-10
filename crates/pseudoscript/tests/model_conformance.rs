//! Model ↔ implementation conformance guard.
//!
//! The facade pattern (publish the contract on the system/container, never reach
//! into an internal `component` from another module) is enforced here on both
//! sides, so a future edit cannot silently reintroduce the reach-ins that phase-1
//! removed:
//!
//! * [`facade_pattern_holds_in_every_workspace`] — driven by the resolved graph of
//!   the self-model and every sample workspace: no cross-module call may target a
//!   `component`; it must land on a `container` or `system` face. Because it reads
//!   the resolved graph, it tracks the model with no hard-coded node list.
//! * [`crate_facades_exist`] — each crate still exposes the public entry the model
//!   names as its container face (a curated correspondence, since the model uses
//!   C4-level names that differ from the Rust symbols, e.g. `render` ↔ `render_svg`).
//! * [`engines_stay_encapsulated`] — the engines the model keeps as private
//!   `component`s are not reachable from another crate (no `pub` type, no re-export).

use std::fs;
use std::path::{Path, PathBuf};

use pseudoscript_model::{EdgeKind, Graph, NodeKind, graph};
use pseudoscript_project::load_modules;

/// Buildable `.pds` workspaces whose facade discipline this guard enforces.
const WORKSPACES: &[&str] = &[
    "model",
    "web-ide/src/lib/samples/acme-payments",
    "web-ide/src/lib/samples/acme-tickets",
    "web-ide/src/lib/samples/banking",
];

/// Each crate's published face — the public symbol(s) the model names as its
/// container face. Curated because the model is C4-level: names need not mirror
/// the Rust symbol (`load` ↔ `load_modules`, `render` ↔ `render_svg`/`render_site`).
const FACADES: &[(&str, &[&str])] = &[
    ("pseudoscript-syntax", &["parse", "render_tokens"]),
    (
        "pseudoscript-model",
        &[
            "graph",
            "check",
            "check_workspace",
            "check_workspace_modules",
        ],
    ),
    ("pseudoscript-format", &["format"]),
    ("pseudoscript-emit", &["project", "render_svg"]),
    ("pseudoscript-dot", &["layout"]),
    ("pseudoscript-layout", &["layout"]),
    ("pseudoscript-doc", &["render_site"]),
    (
        "pseudoscript-lsp-core",
        &["hover", "definition", "format_edit"],
    ),
    ("pseudoscript-project", &["find_root", "load_modules"]),
    (
        "pseudoscript-universe",
        &["build", "from_model", "snapshot", "flows"],
    ),
    ("pseudoscript-lsp", &["run_stdio"]),
    ("pseudoscript-ide", &["mount", "set_source"]),
];

/// Engines the model keeps private behind a crate's facade. They must not be
/// `pub` nor re-exported, so no other crate can name them.
const ENGINES: &[(&str, &[&str])] = &[
    ("pseudoscript-model", &["Builder"]),
    ("pseudoscript-syntax", &["Lexer", "Parser"]),
];

/// The repository root (two levels up from this crate's manifest).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root resolves")
}

/// Resolve a `.pds` workspace at `root` into its graph.
fn graph_of(root: &Path) -> Graph {
    let modules =
        load_modules(root).unwrap_or_else(|e| panic!("load workspace {}: {e}", root.display()));
    graph(&modules)
}

/// All Rust source under `crates/<krate>/src`, concatenated.
fn crate_sources(root: &Path, krate: &str) -> String {
    fn walk(dir: &Path, out: &mut String) {
        let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push_str(&fs::read_to_string(&path).expect("read .rs source"));
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(&root.join("crates").join(krate).join("src"), &mut out);
    out
}

/// Whether `src` declares `pub <keyword> <name>` as a whole identifier (so
/// `Builder` does not match `BuilderState`).
fn declares_pub(src: &str, keyword: &str, name: &str) -> bool {
    let prefix = format!("pub {keyword} {name}");
    src.match_indices(&prefix).any(|(idx, _)| {
        src[idx + prefix.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
    })
}

#[test]
fn facade_pattern_holds_in_every_workspace() {
    let root = workspace_root();
    let mut violations = Vec::new();

    for ws in WORKSPACES {
        let g = graph_of(&root.join(ws));
        for edge in g.edges() {
            if edge.kind != EdgeKind::Call {
                continue;
            }
            let (Some(from), Some(to)) = (g.node(&edge.from), g.node(&edge.to)) else {
                continue;
            };
            if to.kind == NodeKind::Component && from.module != to.module {
                violations.push(format!(
                    "{ws}: {}.{} reaches component {} from module `{}` — publish the contract on a container/system",
                    edge.from, edge.label, edge.to, from.module
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "cross-module component reach-ins found ({}):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn crate_facades_exist() {
    let root = workspace_root();
    let mut missing = Vec::new();

    for (krate, fns) in FACADES {
        let src = crate_sources(&root, krate);
        for name in *fns {
            if !declares_pub(&src, "fn", name) && !declares_pub(&src, "async fn", name) {
                missing.push(format!(
                    "{krate}: the model names this as a container face, but no `pub fn {name}` exists"
                ));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "missing crate facades ({}):\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn engines_stay_encapsulated() {
    let root = workspace_root();
    let mut leaked = Vec::new();

    for (krate, engines) in ENGINES {
        let src = crate_sources(&root, krate);
        for engine in *engines {
            if declares_pub(&src, "struct", engine) || declares_pub(&src, "enum", engine) {
                leaked.push(format!(
                    "{krate}: engine `{engine}` is `pub` — keep it private behind the crate facade"
                ));
            }
            let reexported = src
                .lines()
                .filter(|line| line.trim_start().starts_with("pub use"))
                .any(|line| {
                    line.split(|c: char| !c.is_alphanumeric() && c != '_')
                        .any(|token| token == *engine)
                });
            if reexported {
                leaked.push(format!(
                    "{krate}: engine `{engine}` is re-exported via `pub use`"
                ));
            }
        }
    }

    assert!(
        leaked.is_empty(),
        "engines leaked into a public crate API ({}):\n{}",
        leaked.len(),
        leaked.join("\n")
    );
}
