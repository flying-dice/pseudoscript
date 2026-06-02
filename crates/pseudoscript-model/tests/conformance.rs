//! Conformance harness: runs the `CONFORMANCE/static/` cases plus focused
//! per-rule scenarios as cucumber features.
//!
//! Each static fixture asserts that the set of *error* messages from
//! [`pseudoscript_model::check`] equals the set of non-empty lines in the
//! sibling `.diagnostics` golden (order-independent; an empty golden means no
//! errors). Focused scenarios document each rule with a minimal inline source.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use cucumber::{World, gherkin, given, then, when};
use pseudoscript_model::{
    EdgeKind, Graph, Step, Visibility, WorkspaceModule, check, check_workspace, graph,
};

/// Absolute path to the workspace `CONFORMANCE/` directory.
fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../CONFORMANCE")
        .canonicalize()
        .expect("CONFORMANCE directory exists")
}

/// A stable worked model the graph scenarios assert against. Kept inline (rather
/// than reading the top-level `pseudoscript.pds`, which evolves with the real
/// architecture) so these tests pin the graph builder's behaviour, not the
/// contents of an example file.
const WORKED_MODEL: &str = r"//! pseudoscript

public person Developer;

public data Request { source: string }
public data Ast { module: string }
public data Graph { nodes: number }
public data Diagram { body: string }
public data Failure { message: string }

public system Pseudoscript;

public container Cli for Pseudoscript;
public container Syntax for Pseudoscript;
public container Model for Pseudoscript;
public container Emit for Pseudoscript;

component Generate for Cli {
  #[manual]
  public run(req: Request): Result<Diagram, Failure> {
    ast: Result<Ast, Failure> = Syntax::Parser.parse(req)
    if (ast.isErr) {
      return Err(ast.error)
    }
    graph: Result<Graph, Failure> = Model::Builder.build(ast.value)
    if (graph.isErr) {
      return Err(graph.error)
    }
    sub: Graph = Model::Views.extract(graph.value)
    out: Result<Diagram, Failure> = Emit::Transpiler.emit(sub)
    if (out.isErr) {
      return Err(out.error)
    }
    return Ok(out.value)
  }
}

component Args for Cli {
  parse(argv: string): Request;
}

component Parser for Syntax {
  parse(req: Request): Result<Ast, Failure>;
}

component Builder for Model {
  build(ast: Ast): Result<Graph, Failure>;
}

component Views for Model {
  extract(graph: Graph): Graph;
}

component Transpiler for Emit {
  emit(sub: Graph): Result<Diagram, Failure>;
}
";

/// Lists `*.pds` files in `dir`, sorted.
fn pds_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .expect("readable conformance dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pds"))
        .collect();
    files.sort();
    files
}

/// The set of error-diagnostic messages produced for `src`.
fn error_messages(src: &str) -> BTreeSet<String> {
    check(src)
        .into_iter()
        .filter(pseudoscript_model::Diagnostic::is_error)
        .map(|d| d.message)
        .collect()
}

/// The set of non-empty lines in a `.diagnostics` golden.
fn golden_set(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, Default, World)]
struct ModelWorld {
    messages: BTreeSet<String>,
    failures: Vec<String>,
    /// Modules collected from a `Given the workspace modules:` data table.
    modules: Vec<WorkspaceModule>,
    /// The graph built by a `Given the graph of ...` step.
    graph: Option<Graph>,
}

impl ModelWorld {
    fn graph(&self) -> &Graph {
        self.graph.as_ref().expect("a graph was built")
    }
}

// --- static conformance ----------------------------------------------------

#[when("I check every static conformance fixture")]
fn check_static(world: &mut ModelWorld) {
    let dir = conformance_dir().join("static");
    for pds in pds_files(&dir) {
        let src = fs::read_to_string(&pds).expect("readable .pds");
        let golden = fs::read_to_string(pds.with_extension("diagnostics")).unwrap_or_default();
        let expected = golden_set(&golden);
        let actual = error_messages(&src);
        if actual != expected {
            world.failures.push(format!(
                "{}:\n  expected: {:?}\n  actual:   {:?}",
                pds.file_name().unwrap().to_string_lossy(),
                expected,
                actual
            ));
        }
    }
}

#[then("every fixture's error set equals its golden")]
fn assert_static(world: &mut ModelWorld) {
    assert!(
        world.failures.is_empty(),
        "static conformance mismatches:\n{}",
        world.failures.join("\n")
    );
}

// --- no false positives -----------------------------------------------------

#[when("I check the worked-example fixture and the worked model")]
fn check_well_formed(world: &mut ModelWorld) {
    let worked = conformance_dir().join("static/0-ok-worked-example.pds");
    let fixture = fs::read_to_string(&worked).expect("readable model");
    let cases = [
        ("0-ok-worked-example.pds", fixture.as_str()),
        ("worked model", WORKED_MODEL),
    ];
    for (name, src) in cases {
        let actual = error_messages(src);
        if !actual.is_empty() {
            world
                .failures
                .push(format!("{name}: unexpected errors {actual:?}"));
        }
    }
}

#[then("neither produces an error diagnostic")]
fn assert_well_formed(world: &mut ModelWorld) {
    assert!(
        world.failures.is_empty(),
        "unexpected false positives:\n{}",
        world.failures.join("\n")
    );
}

// --- focused per-rule scenarios ---------------------------------------------

#[given(regex = r"^the model file:$")]
fn given_model(world: &mut ModelWorld, step: &cucumber::gherkin::Step) {
    let src = step.docstring().expect("docstring source");
    world.messages = error_messages(src);
}

// cucumber binds a regex capture to a `FromStr` value, so the step arg must be
// owned `String`, not `&str`.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the diagnostics include "(.+)"$"#)]
fn includes(world: &mut ModelWorld, message: String) {
    assert!(
        world.messages.contains(&message),
        "expected diagnostic {message:?} not found; got {:?}",
        world.messages
    );
}

#[then("there are no diagnostics")]
fn no_diagnostics(world: &mut ModelWorld) {
    assert!(
        world.messages.is_empty(),
        "expected no diagnostics; got {:?}",
        world.messages
    );
}

#[then(regex = r"^there is exactly (\d+) diagnostic$")]
fn exactly_n(world: &mut ModelWorld, n: usize) {
    assert_eq!(
        world.messages.len(),
        n,
        "expected {n} diagnostic(s); got {:?}",
        world.messages
    );
}

// --- cross-module workspace -------------------------------------------------

#[given("the workspace modules:")]
fn given_workspace(world: &mut ModelWorld, step: &gherkin::Step) {
    let table = step.table().expect("a modules table");
    // First row is the header (`fqn | source`); each later row is one module.
    world.modules = table
        .rows
        .iter()
        .skip(1)
        .map(|row| WorkspaceModule::new(row[0].clone(), row[1].replace("\\n", "\n")))
        .collect();
}

#[when("I check the workspace")]
fn when_check_workspace(world: &mut ModelWorld) {
    world.messages = check_workspace(&world.modules)
        .into_iter()
        .filter(pseudoscript_model::Diagnostic::is_error)
        .map(|d| d.message)
        .collect();
}

#[then("the workspace has no errors")]
fn workspace_no_errors(world: &mut ModelWorld) {
    assert!(
        world.messages.is_empty(),
        "expected no errors; got {:?}",
        world.messages
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the workspace diagnostics include "(.+)"$"#)]
fn workspace_includes(world: &mut ModelWorld, message: String) {
    assert!(
        world.messages.contains(&message),
        "expected diagnostic {message:?}; got {:?}",
        world.messages
    );
}

// --- resolved graph ---------------------------------------------------------

#[given("the graph of the top-level model")]
fn given_root_graph(world: &mut ModelWorld) {
    world.graph = Some(graph(&[WorkspaceModule::new("pseudoscript", WORKED_MODEL)]));
}

#[given(regex = r"^the graph of the model:$")]
fn given_inline_graph(world: &mut ModelWorld, step: &gherkin::Step) {
    let src = step.docstring().expect("docstring source").trim_start();
    let fqn = module_fqn(src);
    world.graph = Some(graph(&[WorkspaceModule::new(fqn, src.to_owned())]));
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^node "(.+)" has kind "(.+)"$"#)]
fn node_has_kind(world: &mut ModelWorld, fqn: String, kind: String) {
    let node = world.graph().node(&fqn).expect("node exists");
    assert_eq!(node.kind.keyword(), kind, "kind of {fqn}");
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^node "(.+)" has parent "(.+)"$"#)]
fn node_has_parent(world: &mut ModelWorld, fqn: String, parent: String) {
    let node = world.graph().node(&fqn).expect("node exists");
    assert_eq!(
        node.parent.as_deref(),
        Some(parent.as_str()),
        "parent of {fqn}"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^node "(.+)" is public$"#)]
fn node_is_public(world: &mut ModelWorld, fqn: String) {
    let node = world.graph().node(&fqn).expect("node exists");
    assert_eq!(node.visibility, Visibility::Public, "{fqn} visibility");
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^node "(.+)" is private$"#)]
fn node_is_private(world: &mut ModelWorld, fqn: String) {
    let node = world.graph().node(&fqn).expect("node exists");
    assert_eq!(node.visibility, Visibility::Private, "{fqn} visibility");
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^node "(.+)" has trigger initiator "(.+)"$"#)]
fn node_trigger_initiator(world: &mut ModelWorld, fqn: String, initiator: String) {
    let node = world.graph().node(&fqn).expect("node exists");
    let inits: Vec<String> = node
        .triggers
        .iter()
        .map(pseudoscript_model::Trigger::initiator)
        .collect();
    assert!(
        inits.contains(&initiator),
        "expected initiator {initiator:?} on {fqn}; got {inits:?}"
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^there is a "([^"]+)" edge from "([^"]+)" to "([^"]+)"$"#)]
fn edge_exists(world: &mut ModelWorld, kind: String, from: String, to: String) {
    assert!(
        find_edge(world.graph(), &kind, &from, &to, None),
        "no {kind} edge {from} -> {to}; edges: {:?}",
        world.graph().edges()
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^there is a "([^"]+)" edge from "([^"]+)" to "([^"]+)" labelled "([^"]+)"$"#)]
fn edge_labelled(world: &mut ModelWorld, kind: String, from: String, to: String, label: String) {
    assert!(
        find_edge(world.graph(), &kind, &from, &to, Some(&label)),
        "no {kind} edge {from} -> {to} [{label}]; edges: {:?}",
        world.graph().edges()
    );
}

#[then(regex = r#"^the trace of "(.+)" is:$"#)]
#[allow(clippy::needless_pass_by_value)]
fn trace_is(world: &mut ModelWorld, fqn: String, step: &gherkin::Step) {
    let expected = step.docstring().expect("expected trace").trim();
    let body = world.graph().body(&fqn).expect("callable has a trace");
    let actual = render_trace(body);
    assert_eq!(actual.trim(), expected, "trace of {fqn}");
}

/// Whether an edge of the given `kind` (and optional `label`) connects `from`
/// to `to`.
fn find_edge(g: &Graph, kind: &str, from: &str, to: &str, label: Option<&str>) -> bool {
    let want = match kind {
        "forparent" => EdgeKind::ForParent,
        "call" => EdgeKind::Call,
        "trigger" => EdgeKind::Trigger,
        "provenance" => EdgeKind::Provenance,
        other => panic!("unknown edge kind {other:?}"),
    };
    g.edges().iter().any(|e| {
        e.kind == want && e.from == from && e.to == to && label.is_none_or(|l| e.label == l)
    })
}

/// Renders a sequence trace in the indented step form the goldens assert.
fn render_trace(steps: &[Step]) -> String {
    let mut out = String::new();
    render_steps(steps, 0, &mut out);
    out
}

fn render_steps(steps: &[Step], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    for step in steps {
        match step {
            Step::Call { target_fqn, method } => {
                let _ = writeln!(out, "{pad}call {target_fqn}.{method}");
            }
            Step::SelfCall { method } => {
                let _ = writeln!(out, "{pad}self.{method}");
            }
            Step::Return { marker } if marker.is_empty() => {
                let _ = writeln!(out, "{pad}return");
            }
            Step::Return { marker } => {
                let _ = writeln!(out, "{pad}return {marker}");
            }
            Step::Alt {
                cond_label,
                then,
                r#else,
            } => {
                let _ = writeln!(out, "{pad}alt ({cond_label})");
                render_steps(then, indent + 1, out);
                if !r#else.is_empty() {
                    let _ = writeln!(out, "{pad}else");
                    render_steps(r#else, indent + 1, out);
                }
            }
            Step::Loop { cond_label, body } => {
                let _ = writeln!(out, "{pad}loop ({cond_label})");
                render_steps(body, indent + 1, out);
            }
        }
    }
}

/// The module FQN from a source's first `//!` inner-doc token, matching the
/// single-file convention (`crate::Model::build`).
fn module_fqn(src: &str) -> String {
    src.lines()
        .find_map(|l| l.trim().strip_prefix("//!"))
        .and_then(|d| d.split_whitespace().next())
        .unwrap_or("")
        .to_owned()
}

fn main() {
    futures::executor::block_on(ModelWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
