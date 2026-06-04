//! Generation conformance harness for `pseudoscript-emit`.
//!
//! For every `CONFORMANCE/generation/*.pds`, for each sibling `*.<view>.scene`
//! golden, this projects the named view (deriving `of`/`entry` from the golden's
//! header lines) and asserts `Scene::to_golden()` equals the golden byte-for-byte.
//! SVG smoke scenarios assert each renderer emits well-formed SVG containing the
//! expected labels, without pinning pixels.

use std::fs;
use std::path::{Path, PathBuf};

use cucumber::{World, given, then, when};
use pseudoscript_emit::{Scene, View, graph_of_source, project, render_svg};

/// Absolute path to the `CONFORMANCE/generation` directory.
fn generation_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../CONFORMANCE/generation")
        .canonicalize()
        .expect("CONFORMANCE/generation exists")
}

/// Lists `*.pds` files in `dir`, sorted.
fn pds_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .expect("readable generation dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pds"))
        .collect();
    files.sort();
    files
}

/// The `*.<view>.scene` goldens beside a `.pds` fixture, sorted.
fn scene_goldens(pds: &Path) -> Vec<PathBuf> {
    let stem = pds
        .file_stem()
        .expect("a stem")
        .to_string_lossy()
        .to_string();
    let dir = pds.parent().expect("a parent");
    let mut goldens: Vec<_> = fs::read_dir(dir)
        .expect("readable dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "scene")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&format!("{stem}.")))
        })
        .collect();
    goldens.sort();
    goldens
}

/// Parses a golden's header lines into the [`View`] it asks for.
///
/// The first line is `view <kind>`; a `container`/`component`/`sequence` golden
/// carries the target on the following `of`/`entry` line.
fn view_of_golden(text: &str) -> View {
    let mut lines = text.lines();
    let view = lines
        .next()
        .and_then(|l| l.strip_prefix("view "))
        .expect("a `view` header")
        .trim();
    let mut target = || {
        lines
            .next()
            .and_then(|l| l.split_once(' '))
            .map(|(_, v)| v.trim().to_owned())
            .expect("an `of`/`entry` line")
    };
    match view {
        "context" => View::Context,
        "container" => View::Container { of: target() },
        "component" => View::Component { of: target() },
        "sequence" => View::Sequence { entry: target() },
        // A `data` golden's second line is `of <FQN>`; a `feature` golden's is
        // `entry <FQN>` — both read by `target()`.
        "data" => View::Data { of: target() },
        "feature" => View::Feature { of: target() },
        other => panic!("unknown view {other:?}"),
    }
}

#[derive(Debug, Default, World)]
struct EmitWorld {
    failures: Vec<String>,
    graph: Option<pseudoscript_model::Graph>,
    scene: Option<Scene>,
}

// --- generation conformance -------------------------------------------------

#[when("I project every generation fixture against its scene goldens")]
fn project_all(world: &mut EmitWorld) {
    for pds in pds_files(&generation_dir()) {
        let source = fs::read_to_string(&pds).expect("readable .pds");
        let graph = graph_of_source(&source);
        for golden_path in scene_goldens(&pds) {
            let golden = fs::read_to_string(&golden_path).expect("readable golden");
            let view = view_of_golden(&golden);
            match project(&graph, view) {
                Ok(scene) => {
                    let actual = scene.to_golden();
                    if actual != golden {
                        world.failures.push(format!(
                            "{}:\n--- expected ---\n{golden}--- actual ---\n{actual}",
                            golden_path.file_name().unwrap().to_string_lossy(),
                        ));
                    }
                }
                Err(err) => world.failures.push(format!(
                    "{}: projection failed: {err}",
                    golden_path.file_name().unwrap().to_string_lossy(),
                )),
            }
        }
    }
}

#[then("every projected scene equals its golden byte-for-byte")]
fn assert_goldens(world: &mut EmitWorld) {
    assert!(
        world.failures.is_empty(),
        "generation conformance mismatches:\n{}",
        world.failures.join("\n\n")
    );
}

// --- SVG smoke --------------------------------------------------------------

#[given(regex = r"^the model:$")]
fn given_model(world: &mut EmitWorld, step: &cucumber::gherkin::Step) {
    let source = step.docstring().expect("a docstring model");
    world.graph = Some(graph_of_source(source));
}

#[allow(clippy::needless_pass_by_value)]
#[when(regex = r#"^I project the "([^"]+)" view(?: of "([^"]*)")?$"#)]
fn project_view(world: &mut EmitWorld, view: String, target: String) {
    let view = match view.as_str() {
        "context" => View::Context,
        "container" => View::Container { of: target },
        "component" => View::Component { of: target },
        "sequence" => View::Sequence { entry: target },
        other => panic!("unknown view {other:?}"),
    };
    let scene = project(world.graph(), view).expect("projection succeeds");
    world.scene = Some(scene);
}

#[then("the rendered SVG is well-formed")]
fn svg_well_formed(world: &mut EmitWorld) {
    let svg = render_svg(world.scene());
    assert!(svg.starts_with("<svg"), "SVG starts with <svg: {svg}");
    assert!(svg.trim_end().ends_with("</svg>"), "SVG closes its root");
    assert_eq!(
        svg.matches("<svg").count(),
        svg.matches("</svg>").count(),
        "balanced <svg> tags"
    );
}

#[then("the rendered SVG is identical across two renders")]
fn svg_deterministic(world: &mut EmitWorld) {
    let first = render_svg(world.scene());
    let second = render_svg(world.scene());
    assert_eq!(first, second, "render_svg is deterministic");
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the rendered SVG contains "([^"]*)"$"#)]
fn svg_contains(world: &mut EmitWorld, needle: String) {
    let svg = render_svg(world.scene());
    assert!(svg.contains(&needle), "SVG contains {needle:?}: {svg}");
}

// --- world helpers ----------------------------------------------------------

impl EmitWorld {
    fn graph(&self) -> &pseudoscript_model::Graph {
        self.graph.as_ref().expect("a model graph")
    }

    fn scene(&self) -> &Scene {
        self.scene.as_ref().expect("a projected scene")
    }
}

fn main() {
    futures::executor::block_on(EmitWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
