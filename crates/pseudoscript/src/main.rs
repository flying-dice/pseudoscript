//! `pds` — the `PseudoScript` command-line driver.
//!
//! Subcommands wrap the workspace libraries:
//!
//! - `pds lang` — print the bundled language reference (spec + patterns + the
//!   conformance suite), embedded at compile time.
//! - `pds skill` — print the bundled authoring skill.
//! - `pds lsp` — start the language server over stdio ([`pseudoscript_lsp`]).
//! - `pds check <FILE>` — report diagnostics; exit non-zero on any error.
//! - `pds eval` — read a model from stdin and report diagnostics; for agents
//!   checking a snippet without writing a file.
//! - `pds fmt <FILE> [--write]` — canonical formatting.
//! - `pds tokens <FILE>` — the lexical token stream, for debugging.
//! - `pds doc [PATH]` — generate the documentation site
//!   ([`pseudoscript_doc`]) for the workspace rooted at the nearest
//!   `pds.toml`.
//! - `pds init [PATH]` — bootstrap a new workspace (`pds.toml` + a starter module).
//! - `pds upgrade [VERSION]` — replace the binary with a GitHub release.
//! - `pds add <URL>` — add a git workspace dependency and resolve it ([`deps`]).
//! - `pds install` — restore `pds_modules/` from `pds.lock` ([`deps`]).

mod deps;
mod monorepo;
mod upgrade;
mod workspace;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use pseudoscript_emit::{Theme, View, project, project_symbol, render_svg_themed};
use pseudoscript_format::{FormatError, format};
use pseudoscript_model::{
    Diagnostic, WorkspaceModule, check, check_workspace_with_externals, graph,
};
use pseudoscript_syntax::{LineIndex, Severity, render_tokens};
use serde::Serialize;

/// The full authoring reference (spec + patterns + conformance suite), assembled
/// by `build.rs` and embedded so `pds lang` always matches the installed binary.
const LANG_BUNDLE: &str = include_str!(concat!(env!("OUT_DIR"), "/lang-bundle.md"));

/// The `PseudoScript` authoring-method skill, embedded verbatim so `pds skill`
/// prints it like any other skill file.
const PDS_SKILL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.claude/skills/pseudocode/SKILL.md"
));

/// The `PseudoScript` toolchain.
#[derive(Debug, Parser)]
#[command(
    name = "pds",
    version,
    about = "PseudoScript toolchain",
    after_help = "Writing .pds files? Run `pds lang` for the full language \
reference (spec + patterns + grammar suite), or `pds skill` for the authoring \
method. New here? `pds init` scaffolds a workspace."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The output format for `pds doc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum DocFormat {
    /// The interactive Svelte HTML site (default).
    Html,
    /// Static Markdown files with the diagrams inlined as self-contained SVG.
    Md,
}

/// The `pds` subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Bootstrap a new workspace: write `pds.toml` and a starter module.
    Init {
        /// Directory to initialize; created if absent. Defaults to the current
        /// directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Project name written to `pds.toml`. Defaults to the directory name.
        #[arg(long)]
        name: Option<String>,
    },
    /// Print the bundled language reference (spec, patterns, and the full
    /// conformance/grammar suite) for this exact version. Feed it to an LLM to
    /// author .pds.
    #[command(alias = "spec")]
    Lang,
    /// Print the bundled authoring skill — the method for modelling with .pds.
    /// Run `pds lang` for the full grammar.
    Skill,
    /// Start the language server over stdio.
    Lsp,
    /// Check a file and report diagnostics (exit non-zero on any error).
    Check {
        /// The `.pds` file to check — or, with `--all`, a root directory to
        /// discover workspaces under. Defaults to the current directory.
        #[arg(default_value = ".")]
        file: PathBuf,
        /// Check every `pds.toml` workspace discovered under the path, with
        /// cross-workspace dependency resolution. Exits non-zero if any fails.
        #[arg(long)]
        all: bool,
    },
    /// Read a model from stdin and report diagnostics (exit non-zero on any
    /// error). For agents: pipe a snippet to rapidly check it parses and
    /// resolves, without writing a file — e.g. `pds eval < model.pds`.
    Eval,
    /// Format a file to canonical `PseudoScript`.
    Fmt {
        /// The `.pds` file to format.
        file: PathBuf,
        /// Overwrite the file in place instead of printing to stdout.
        #[arg(long)]
        write: bool,
    },
    /// Print the lexical token stream for a file.
    Tokens {
        /// The `.pds` file to tokenize.
        file: PathBuf,
    },
    /// Print the workspace's symbol outline as JSON — the structure tree an
    /// editor draws (each node's `fqn`, `name`, `kind`, `parent`, whether it is
    /// a `triggered` flow entry, and its declaration `module`/`line`/`col`).
    Outline {
        /// A file or directory inside the workspace; the project root is the
        /// nearest enclosing `pds.toml`. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Render a single diagram to a self-contained SVG on stdout. With
    /// `--symbol`, draws that symbol's fitting view (a system/container's C4
    /// sub-view, or a triggered callable's sequence); otherwise draws `--view`
    /// over the whole workspace.
    Svg {
        /// A file or directory inside the workspace; the project root is the
        /// nearest enclosing `pds.toml`. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Render the fitting view for this symbol FQN (e.g. `acme::Billing`).
        #[arg(long, conflicts_with = "view")]
        symbol: Option<String>,
        /// Render a whole-workspace view: `context`, or `container`/`component`/
        /// `sequence`/`data`/`feature` paired with `--target`. Defaults to
        /// `context`.
        #[arg(long, conflicts_with = "symbol")]
        view: Option<String>,
        /// The target FQN a non-`context` view is `of`.
        #[arg(long, requires = "view")]
        target: Option<String>,
        /// Colour theme: `light` (default) or `dark`.
        #[arg(long, default_value = "light")]
        theme: String,
    },
    /// Generate the documentation site for the workspace.
    Doc {
        /// A file or directory inside the workspace; the project root is the
        /// nearest enclosing `pds.toml`. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// After generating, serve the site over HTTP and block until stopped.
        #[arg(long)]
        serve: bool,
        /// Watch the workspace, regenerate on change, and live-reload the
        /// browser. Implies `--serve`.
        #[arg(long)]
        watch: bool,
        /// Port for the server (with `--serve` or `--watch`).
        #[arg(long, default_value_t = 8000)]
        port: u16,
        /// Generate docs for every `pds.toml` workspace discovered under the
        /// path. Incompatible with `--serve`/`--watch`.
        #[arg(long, conflicts_with_all = ["serve", "watch"])]
        all: bool,
        /// Output format: the interactive HTML site (`html`), or static Markdown
        /// files with the diagrams inlined as SVG (`md`). Overrides
        /// `[doc].format` in `pds.toml`; defaults to that, else `html`. Markdown
        /// is generate-only — `--serve`/`--watch` apply to `html`.
        #[arg(long, value_enum)]
        format: Option<DocFormat>,
    },
    /// Download and install a release over the running binary.
    Upgrade {
        /// Release to install, e.g. `0.1.0` or `v0.1.0`. Defaults to the latest.
        version: Option<String>,
    },
    /// Add a git workspace dependency to `pds.toml` and resolve it.
    Add {
        /// The dependency's git URL.
        url: String,
        /// Install a specific tag.
        #[arg(long, conflicts_with_all = ["rev", "branch"])]
        tag: Option<String>,
        /// Install a specific commit.
        #[arg(long, conflicts_with_all = ["tag", "branch"])]
        rev: Option<String>,
        /// Install the tip of a branch.
        #[arg(long, conflicts_with_all = ["tag", "rev"])]
        branch: Option<String>,
        /// The dependency workspace's directory within the repo (default: root).
        #[arg(long)]
        path: Option<String>,
        /// The dependency name (its FQN root). Defaults to the repo name.
        #[arg(long)]
        name: Option<String>,
    },
    /// Restore `pds_modules/` from `pds.lock`.
    Install,
    /// Re-resolve git dependencies, repinning `pds.lock` to current commits.
    Update,
    /// Remove a dependency from `pds.toml` and `pds.lock`.
    Remove {
        /// The dependency name to remove.
        name: String,
    },
    /// List every `pds.toml` workspace under a root directory.
    List {
        /// The root to search. Defaults to the current directory.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path, name } => cmd_init(&path, name.as_deref()),
        Command::Lang => cmd_lang(),
        Command::Skill => cmd_skill(),
        Command::Lsp => run_lsp(),
        Command::Check { file, all } => {
            if all {
                cmd_check_all(&file)
            } else {
                cmd_check(&file)
            }
        }
        Command::Eval => cmd_eval(),
        Command::Fmt { file, write } => cmd_fmt(&file, write),
        Command::Tokens { file } => cmd_tokens(&file),
        Command::Outline { path } => cmd_outline(&path),
        Command::Svg {
            path,
            symbol,
            view,
            target,
            theme,
        } => cmd_svg(
            &path,
            symbol.as_deref(),
            view.as_deref(),
            target.as_deref(),
            &theme,
        ),
        Command::Doc {
            path,
            serve,
            watch,
            port,
            all,
            format,
        } => {
            if all {
                cmd_doc_all(&path, format)
            } else {
                cmd_doc(&path, serve, watch, port, format)
            }
        }
        Command::Upgrade { version } => cmd_upgrade(version),
        Command::Add {
            url,
            tag,
            rev,
            branch,
            path,
            name,
        } => cmd_add(&url, tag, rev, branch, path, name),
        Command::Install => cmd_install(),
        Command::Update => report(deps::update(Path::new("."))),
        Command::Remove { name } => report(deps::remove(Path::new("."), &name)),
        Command::List { root } => cmd_list(&root),
    }
}

/// `pds lang`: print the embedded reference bundle verbatim to stdout. The blob
/// already ends in a newline, so `print!` reproduces it byte-for-byte.
fn cmd_lang() -> ExitCode {
    print!("{LANG_BUNDLE}");
    ExitCode::SUCCESS
}

/// `pds skill`: print the embedded authoring skill. The source has no trailing
/// newline, so add one for a clean terminal.
fn cmd_skill() -> ExitCode {
    println!("{}", PDS_SKILL.trim_end_matches('\n'));
    ExitCode::SUCCESS
}

/// `pds upgrade`: download and install a release over the running binary.
fn cmd_upgrade(version: Option<String>) -> ExitCode {
    report(upgrade::run(version))
}

/// `pds add`: resolve a git dependency into `pds.toml` and `pds.lock`.
fn cmd_add(
    url: &str,
    tag: Option<String>,
    rev: Option<String>,
    branch: Option<String>,
    path: Option<String>,
    name: Option<String>,
) -> ExitCode {
    let result = deps::Rev::from_flags(tag, rev, branch)
        .and_then(|selector| deps::add(Path::new("."), url, &selector, path, name));
    report(result)
}

/// `pds install`: restore `pds_modules/` from `pds.lock`.
fn cmd_install() -> ExitCode {
    report(deps::install(Path::new(".")))
}

/// `pds list`: print every discovered `pds.toml` workspace under `root`.
fn cmd_list(root: &Path) -> ExitCode {
    match monorepo::discover(root) {
        Ok(roots) if roots.is_empty() => {
            eprintln!("no `pds.toml` workspace found under {}", root.display());
            ExitCode::FAILURE
        }
        Ok(roots) => {
            for ws in roots {
                println!("{}", ws.display());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// `pds check --all`: check every discovered workspace under `root`, resolving
/// each workspace's declared dependencies. Exits non-zero if any workspace has
/// an error-severity diagnostic or fails to load.
fn cmd_check_all(root: &Path) -> ExitCode {
    let roots = match monorepo::discover(root) {
        Ok(roots) if roots.is_empty() => {
            eprintln!("no `pds.toml` workspace found under {}", root.display());
            return ExitCode::FAILURE;
        }
        Ok(roots) => roots,
        Err(err) => {
            eprintln!("error: {err:#}");
            return ExitCode::FAILURE;
        }
    };

    let mut had_error = false;
    for ws in &roots {
        println!("checking {}", ws.display());
        had_error |= check_one_workspace(ws);
    }
    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Loads and checks one workspace, printing each diagnostic with its workspace
/// path. Returns whether the workspace produced an error (a load failure or an
/// error-severity diagnostic).
fn check_one_workspace(root: &Path) -> bool {
    let project = match workspace::load(root) {
        Ok(project) => project,
        Err(err) => {
            eprintln!("error: {}: {err:#}", root.display());
            return true;
        }
    };
    let diagnostics = check_workspace_with_externals(&project.modules, &project.dependencies);
    let mut had_error = false;
    for diag in &diagnostics {
        eprintln!(
            "{}: {}: {}",
            root.display(),
            severity_label(diag.severity),
            diag.message
        );
        had_error |= diag.is_error();
    }
    had_error
}

/// `pds doc --all`: generate docs for every discovered workspace under `root`.
/// Exits non-zero if any workspace fails to build.
fn cmd_doc_all(root: &Path, format: Option<DocFormat>) -> ExitCode {
    let roots = match monorepo::discover(root) {
        Ok(roots) if roots.is_empty() => {
            eprintln!("no `pds.toml` workspace found under {}", root.display());
            return ExitCode::FAILURE;
        }
        Ok(roots) => roots,
        Err(err) => {
            eprintln!("error: {err:#}");
            return ExitCode::FAILURE;
        }
    };

    let mut failed = false;
    for ws in &roots {
        if let Err(err) = build_site(ws, true, format) {
            eprintln!("error: {}: {err:#}", ws.display());
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Maps a unit-or-error command result to an exit code, printing the error.
fn report(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Reads `path`, reporting a clear error to stderr if it cannot be read.
fn read(path: &Path) -> Result<String, ExitCode> {
    fs::read_to_string(path).map_err(|err| {
        eprintln!("{}: {err}", path.display());
        ExitCode::FAILURE
    })
}

/// Boots a Tokio runtime and runs the stdio language server.
fn run_lsp() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("failed to start async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(pseudoscript_lsp::run_stdio());
    ExitCode::SUCCESS
}

/// The starter module written by `pds init` — a minimal, well-formed model.
const STARTER_MODULE: &str = "\
//! Top-level model. Run `pds doc` to render its diagrams.

/// The system this workspace models.
public system App {
    /// An example operation.
    start();
}
";

/// `pds init`: write `pds.toml` and a starter `main.pds` into `path`, creating
/// the directory if needed. Refuses to overwrite an existing `pds.toml`.
fn cmd_init(path: &Path, name: Option<&str>) -> ExitCode {
    match run_init(path, name) {
        Ok(created) => {
            for file in &created {
                println!("  created {}", file.display());
            }
            println!("Initialized PseudoScript workspace in {}", path.display());
            println!("Next: `pds check main.pds`, then `pds doc --serve`.");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Creates the workspace files, returning the paths written. The manifest is
/// never overwritten; the starter module is written only if absent.
fn run_init(path: &Path, name: Option<&str>) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(path).with_context(|| format!("creating `{}`", path.display()))?;

    let manifest = path.join("pds.toml");
    if manifest.exists() {
        bail!(
            "`{}` already exists — this directory is already a workspace",
            manifest.display()
        );
    }

    let project_name = match name {
        Some(name) => name.to_owned(),
        None => default_project_name(path),
    };

    let mut created = Vec::new();
    fs::write(&manifest, manifest_contents(&project_name))
        .with_context(|| format!("writing `{}`", manifest.display()))?;
    created.push(manifest);

    let starter = path.join("main.pds");
    if !starter.exists() {
        fs::write(&starter, STARTER_MODULE)
            .with_context(|| format!("writing `{}`", starter.display()))?;
        created.push(starter);
    }
    Ok(created)
}

/// The `pds.toml` body for a new project: a `[doc]` table naming the site.
fn manifest_contents(name: &str) -> String {
    format!("# PseudoScript project root (LANG.md §8.1).\n[doc]\nname = \"{name}\"\n")
}

/// The default project name: the initialized directory's final component,
/// falling back to `workspace` for a path with no usable name (e.g. `/`).
fn default_project_name(path: &Path) -> String {
    path.canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .map_or_else(
            || "workspace".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
}

/// `pds check`: check a single `.pds` file, or — when given a directory — the
/// workspace rooted there (with its declared dependencies resolved). Prints
/// `path:line:col: severity: message` per diagnostic and exits non-zero if any
/// error-severity diagnostic was produced.
fn cmd_check(path: &Path) -> ExitCode {
    if path.is_dir() {
        let root = match workspace::find_root(path) {
            Ok(root) => root,
            Err(err) => {
                eprintln!("error: {err:#}");
                return ExitCode::FAILURE;
            }
        };
        return if check_one_workspace(&root) {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }
    // A file inside a workspace carries a module FQN — its path under `pds.toml` (§8.1) —
    // so check it in that context: the same strict, FQN-resolving check `pds check <dir>`
    // and `pds doc` run. Only a rootless file (no enclosing `pds.toml`) is checked as an
    // anonymous single module, which is lenient about full qualification (ADR-029).
    if let Ok(root) = workspace::find_root(path) {
        return if check_one_workspace(&root) {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }
    let src = match read(path) {
        Ok(src) => src,
        Err(code) => return code,
    };
    report_source(&path.display().to_string(), &src)
}

/// `pds eval`: read a model from stdin and report diagnostics, exiting non-zero
/// on any error. Lets an agent check a snippet's syntax and resolution without
/// touching disk — `pds eval < model.pds` or a piped here-doc.
fn cmd_eval() -> ExitCode {
    match std::io::read_to_string(std::io::stdin()) {
        Ok(src) => report_source("<stdin>", &src),
        Err(err) => {
            eprintln!("<stdin>: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Run `check` over a single source buffer, printing each diagnostic to stderr
/// as `label:line:col: severity: message`. `label` is the file path or
/// `<stdin>`. Returns `FAILURE` if any diagnostic is an error, else `SUCCESS`.
fn report_source(label: &str, src: &str) -> ExitCode {
    let index = LineIndex::new(src);
    let mut had_error = false;
    for diag in &check(src) {
        let (line, col) = index.line_col(diag.span.start);
        eprintln!(
            "{label}:{line}:{col}: {}: {}",
            severity_label(diag.severity),
            diag.message
        );
        had_error |= diag.is_error();
    }
    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// `pds fmt`: print canonical text, or with `--write` overwrite the file. On a
/// parse error, report it and exit non-zero without touching the file.
fn cmd_fmt(path: &Path, write: bool) -> ExitCode {
    let src = match read(path) {
        Ok(src) => src,
        Err(code) => return code,
    };
    match format(&src) {
        Ok(formatted) => {
            if write {
                if let Err(err) = fs::write(path, &formatted) {
                    eprintln!("{}: {err}", path.display());
                    return ExitCode::FAILURE;
                }
            } else {
                print!("{formatted}");
            }
            ExitCode::SUCCESS
        }
        Err(FormatError::Parse(errors)) => {
            eprintln!("{}: cannot format: source has parse errors", path.display());
            for err in errors {
                eprintln!("  {err}");
            }
            ExitCode::FAILURE
        }
    }
}

/// `pds tokens`: print the `KIND@line:col "lexeme"` token stream to stdout.
fn cmd_tokens(path: &Path) -> ExitCode {
    let src = match read(path) {
        Ok(src) => src,
        Err(code) => return code,
    };
    print!("{}", render_tokens(&src));
    ExitCode::SUCCESS
}

/// One entry in `pds outline` — a declared node, for an editor's structure tree.
/// Mirrors the workspace outline the web IDE builds, so the two agree on shape.
#[derive(Serialize)]
struct OutlineNode {
    /// Fully-qualified name (the diagram/navigation key).
    fqn: String,
    /// The bare declared name shown in the tree.
    name: String,
    /// One of `person`/`system`/`container`/`component`/`data`/`callable`.
    kind: &'static str,
    /// The structural parent's FQN, if any (a node may nest under another
    /// module's node, so the tree is workspace-wide).
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    /// True for a callable that triggers a flow — its fitting diagram is a
    /// sequence rather than a C4 view.
    triggered: bool,
    /// The module that declares it, and the 1-based position of its name (so a
    /// host can offer go-to-definition).
    module: String,
    line: u32,
    col: u32,
}

/// Loads every module of the workspace enclosing `path` (the nearest `pds.toml`),
/// reporting a clear error to stderr on failure.
fn workspace_modules(path: &Path) -> Result<Vec<WorkspaceModule>, ExitCode> {
    let fail = |err: anyhow::Error| {
        eprintln!("error: {err:#}");
        ExitCode::FAILURE
    };
    let root = workspace::find_root(path).map_err(fail)?;
    workspace::load_modules(&root).map_err(fail)
}

/// `pds outline`: print the workspace's symbol outline as JSON.
fn cmd_outline(path: &Path) -> ExitCode {
    let modules = match workspace_modules(path) {
        Ok(modules) => modules,
        Err(code) => return code,
    };
    // One line index per module so a node's byte offset maps to its 1-based
    // position in the module that declares it.
    let indices: std::collections::HashMap<&str, LineIndex> = modules
        .iter()
        .map(|m| (m.fqn.as_str(), LineIndex::new(&m.source)))
        .collect();
    let line_col = |module: &str, offset| {
        indices
            .get(module)
            .map_or((1, 1), |idx| idx.line_col(offset))
    };
    let graph = graph(&modules);
    let nodes = graph.nodes().iter().map(|n| {
        let (line, col) = line_col(n.module.as_str(), n.span.start);
        OutlineNode {
            fqn: n.fqn.clone(),
            name: n.name.clone(),
            kind: n.kind.keyword(),
            parent: n.parent.clone(),
            triggered: !n.triggers.is_empty(),
            module: n.module.clone(),
            line,
            col,
        }
    });
    // Features are not graph nodes (§5.2); list each as a `feature` entry nested
    // under its target node, so `pds outline` agrees with the web IDE's outline.
    let features = graph.scenarios().iter().map(|s| {
        let (line, col) = line_col(s.module.as_str(), s.span.start);
        OutlineNode {
            fqn: format!("{}::{}", s.module, s.name),
            name: s.name.clone(),
            kind: "feature",
            parent: Some(s.target_fqn.clone()),
            triggered: false,
            module: s.module.clone(),
            line,
            col,
        }
    });
    let outline: Vec<OutlineNode> = nodes.chain(features).collect();
    match serde_json::to_string_pretty(&outline) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `pds svg`: render one diagram to a self-contained SVG on stdout. `--symbol`
/// draws that symbol's fitting view; otherwise `--view` (+ `--target`) draws a
/// whole-workspace view.
fn cmd_svg(
    path: &Path,
    symbol: Option<&str>,
    view: Option<&str>,
    target: Option<&str>,
    theme: &str,
) -> ExitCode {
    let theme = match theme {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        other => {
            eprintln!("error: unknown theme `{other}` (expected `light` or `dark`)");
            return ExitCode::FAILURE;
        }
    };
    let modules = match workspace_modules(path) {
        Ok(modules) => modules,
        Err(code) => return code,
    };
    let g = graph(&modules);
    let scene = if let Some(fqn) = symbol {
        project_symbol(&g, fqn)
    } else {
        let view = match resolve_view(view.unwrap_or("context"), target.unwrap_or("")) {
            Ok(view) => view,
            Err(err) => {
                eprintln!("error: {err:#}");
                return ExitCode::FAILURE;
            }
        };
        project(&g, view)
    };
    match scene {
        Ok(scene) => {
            print!("{}", render_svg_themed(&scene, theme));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Resolves a `--view`/`--target` pair into an emit [`View`].
fn resolve_view(view: &str, target: &str) -> Result<View> {
    Ok(match view {
        "context" => View::Context,
        "container" => View::Container {
            of: target.to_owned(),
        },
        "component" => View::Component {
            of: target.to_owned(),
        },
        "sequence" => View::Sequence {
            entry: target.to_owned(),
        },
        "data" => View::Data {
            of: target.to_owned(),
        },
        "feature" => View::Feature {
            of: target.to_owned(),
        },
        other => {
            bail!(
                "unknown view `{other}` (expected context/container/component/sequence/data/feature)"
            )
        }
    })
}

/// `pds doc`: generate the documentation site for the workspace rooted at the
/// nearest `pds.toml`.
///
/// Model diagnostics are reported to stderr but, like `cargo doc`, never abort
/// generation — a model with warnings still documents. Only I/O and load errors
/// fail the command.
fn cmd_doc(
    path: &Path,
    serve: bool,
    watch: bool,
    port: u16,
    format: Option<DocFormat>,
) -> ExitCode {
    // Watching only makes sense while serving (regenerate, then live-reload).
    let serve = serve || watch;
    match run_doc(path, serve, watch, port, format) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Generates the site once, then optionally serves it — with a workspace
/// watcher and browser live-reload when `watch` is set.
fn run_doc(
    path: &Path,
    serve: bool,
    watch: bool,
    port: u16,
    format: Option<DocFormat>,
) -> Result<()> {
    let (out_dir, format) = build_site(path, true, format)?;
    if format == DocFormat::Md {
        if serve || watch {
            eprintln!("note: --serve/--watch are not supported for Markdown output");
        }
        return Ok(());
    }
    if !serve {
        return Ok(());
    }
    let version = watch.then(|| Arc::new(AtomicU64::new(1)));
    if let Some(version) = &version {
        spawn_watcher(path.to_owned(), out_dir.clone(), Arc::clone(version));
    }
    serve_docs(&out_dir, port, version.as_deref())
}

/// Loads the workspace, renders the site, and writes it to the output dir,
/// returning that dir. `announce` prints the file count (quieted on rebuilds,
/// which print their own line).
fn build_site(
    path: &Path,
    announce: bool,
    cli_format: Option<DocFormat>,
) -> Result<(PathBuf, DocFormat)> {
    let root = workspace::find_root(path)?;
    let project = workspace::load(&root)?;
    // Precedence: an explicit `--format` wins over `[doc].format`, else HTML.
    let format = cli_format.or(project.doc_format).unwrap_or(DocFormat::Html);

    report_diagnostics(&check_workspace_with_externals(
        &project.modules,
        &project.dependencies,
    ));

    let model = graph(&project.modules);
    let site = match format {
        DocFormat::Html => pseudoscript_doc::try_render_site(&model, &project.config)
            .context("rendering the documentation site")?,
        DocFormat::Md => pseudoscript_doc::render_markdown_site(&model, &project.config),
    };
    for file in &site.files {
        let dest = project.out_dir.join(&file.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating `{}`", parent.display()))?;
        }
        fs::write(&dest, &file.contents)
            .with_context(|| format!("writing `{}`", dest.display()))?;
    }

    // The Markdown output inlines its SVG; only the HTML site references a logo.
    if format == DocFormat::Html {
        copy_logo(&root, &project);
    }

    if announce {
        println!(
            "Generated {} files in {}",
            site.files.len(),
            project.out_dir.display()
        );
    }
    Ok((project.out_dir, format))
}

/// Spawns a detached thread that watches the workspace and regenerates the site
/// on every source change, bumping `version` so connected browsers reload.
fn spawn_watcher(path: PathBuf, out_dir: PathBuf, version: Arc<AtomicU64>) {
    std::thread::spawn(move || {
        if let Err(err) = watch_loop(&path, &out_dir, &version) {
            eprintln!("warning: watch disabled: {err:#}");
        }
    });
}

/// Blocks watching the project root, rebuilding on a debounced burst of changes
/// to `.pds` files or `pds.toml`. Events under `out_dir` (the site we write) are
/// ignored so writing the site never re-triggers a build.
fn watch_loop(path: &Path, out_dir: &Path, version: &AtomicU64) -> Result<()> {
    use notify::{RecursiveMode, Watcher};

    let root = workspace::find_root(path)?;
    let out_dir = out_dir
        .canonicalize()
        .unwrap_or_else(|_| out_dir.to_owned());

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;
    println!("Watching {} for changes…", root.display());

    loop {
        // Block for a change, then drain the burst editors emit (save = several
        // events) so one build covers them all.
        let first = rx.recv()?;
        let mut events = vec![first];
        while let Ok(event) = rx.recv_timeout(Duration::from_millis(200)) {
            events.push(event);
        }
        let relevant = events
            .iter()
            .flatten()
            .flat_map(|event| event.paths.iter())
            .any(|p| is_source_change(p, &out_dir));
        if !relevant {
            continue;
        }
        match build_site(path, false, Some(DocFormat::Html)) {
            Ok(_) => {
                version.fetch_add(1, Ordering::SeqCst);
                println!("↻ regenerated");
            }
            Err(err) => eprintln!("warning: rebuild failed: {err:#}"),
        }
    }
}

/// Whether `p` is a model source whose change should trigger a rebuild: a
/// `.pds` file or `pds.toml`, and not under the generated `out_dir`.
fn is_source_change(p: &Path, out_dir: &Path) -> bool {
    let canon = p.canonicalize();
    let resolved = canon.as_deref().unwrap_or(p);
    if p.starts_with(out_dir) || resolved.starts_with(out_dir) {
        return false;
    }
    p.extension().is_some_and(|ext| ext == "pds")
        || p.file_name().is_some_and(|name| name == "pds.toml")
}

/// The live-reload client: polls `/__livereload` and reloads when the version
/// the server reports changes. Injected into served HTML only in watch mode.
const LIVERELOAD: &str = "<script>(function(){var v=null;function poll(){\
fetch('/__livereload',{cache:'no-store'}).then(function(r){return r.text()})\
.then(function(t){if(v!==null&&t!==v){location.reload();return}v=t;setTimeout(poll,1000)})\
.catch(function(){setTimeout(poll,2000)})}poll()})();</script>";

/// Serves `dir` over HTTP on `127.0.0.1:port` until the process is stopped.
/// Maps `/` to `index.html`; rejects paths that escape `dir`. When `reload` is
/// set, HTML carries the live-reload client and `/__livereload` reports the
/// current site version.
fn serve_docs(dir: &Path, port: u16, reload: Option<&AtomicU64>) -> Result<()> {
    let server = tiny_http::Server::http(("127.0.0.1", port))
        .map_err(|err| anyhow::anyhow!("starting server on port {port}: {err}"))?;
    let note = if reload.is_some() {
        " — live reload on"
    } else {
        ""
    };
    println!("Serving docs at http://127.0.0.1:{port}/{note} (Ctrl-C to stop)");

    for request in server.incoming_requests() {
        let response = build_response(dir, request.url(), reload);
        if let Err(err) = request.respond(response) {
            eprintln!("warning: response failed: {err}");
        }
    }
    Ok(())
}

/// Resolves an HTTP request path to a file response under `dir`. A path that
/// escapes `dir` (or names no file) yields 404; `/` maps to `index.html`. With
/// `reload` set, `/__livereload` reports the version and HTML is instrumented.
fn build_response(
    dir: &Path,
    url: &str,
    reload: Option<&AtomicU64>,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let not_found = || tiny_http::Response::from_string("404 Not Found").with_status_code(404);

    let raw_path = url.split(['?', '#']).next().unwrap_or("");
    if let Some(version) = reload
        && raw_path == "/__livereload"
    {
        return text_response(version.load(Ordering::SeqCst).to_string());
    }

    let Some(path) = safe_rel_path(url) else {
        return not_found();
    };
    let Ok(bytes) = fs::read(dir.join(&path)) else {
        return not_found();
    };
    let content_type = content_type(&path);
    let bytes = if reload.is_some() && content_type.starts_with("text/html") {
        inject_livereload(bytes)
    } else {
        bytes
    };
    let header = tiny_http::Header::from_bytes(b"Content-Type".as_slice(), content_type.as_bytes())
        .expect("static header is valid");
    tiny_http::Response::from_data(bytes).with_header(header)
}

/// A `text/plain`, no-store response (the `/__livereload` version endpoint).
fn text_response(body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let ct = tiny_http::Header::from_bytes(b"Content-Type".as_slice(), b"text/plain".as_slice())
        .expect("static header is valid");
    let cache = tiny_http::Header::from_bytes(b"Cache-Control".as_slice(), b"no-store".as_slice())
        .expect("static header is valid");
    tiny_http::Response::from_string(body)
        .with_header(ct)
        .with_header(cache)
}

/// Inserts the [`LIVERELOAD`] client before `</body>` (or appends it when there
/// is none). Leaves non-UTF-8 bodies untouched.
fn inject_livereload(bytes: Vec<u8>) -> Vec<u8> {
    let mut html = match String::from_utf8(bytes) {
        Ok(html) => html,
        Err(err) => return err.into_bytes(),
    };
    match html.rfind("</body>") {
        Some(idx) => html.insert_str(idx, LIVERELOAD),
        None => html.push_str(LIVERELOAD),
    }
    html.into_bytes()
}

/// The safe, served-root-relative file path for a request URL, or `None` if it
/// escapes the root. The query/fragment is stripped; `/` maps to `index.html`;
/// any `..` or root component is rejected.
fn safe_rel_path(url: &str) -> Option<String> {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    Path::new(path)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
        .then(|| path.to_owned())
}

/// The MIME type for a served file, by extension.
fn content_type(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// Copies the configured logo into the output directory, resolved relative to
/// the project root. A missing logo warns but does not fail (`LANG.md` §9.3).
fn copy_logo(root: &Path, project: &workspace::Workspace) {
    let (Some(source), Some(filename)) = (
        project.config.logo.as_deref(),
        project.config.logo_filename(),
    ) else {
        return;
    };
    let src = root.join(source);
    let dest = project.out_dir.join(filename);
    if let Err(err) = fs::copy(&src, &dest) {
        eprintln!("warning: logo `{}`: {err}", src.display());
    }
}

/// Prints each workspace diagnostic to stderr as `severity: message`. Spans are
/// per-module byte offsets, so no path/line is rendered here.
fn report_diagnostics(diagnostics: &[Diagnostic]) {
    for diag in diagnostics {
        eprintln!("{}: {}", severity_label(diag.severity), diag.message);
    }
}

/// The lowercase label `pds check` prints for a [`Severity`].
fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

#[cfg(test)]
mod tests {
    use super::{content_type, safe_rel_path};
    use std::path::Path;

    #[test]
    fn root_maps_to_index() {
        assert_eq!(safe_rel_path("/").as_deref(), Some("index.html"));
        assert_eq!(safe_rel_path("").as_deref(), Some("index.html"));
    }

    #[test]
    fn strips_query_and_fragment() {
        assert_eq!(
            safe_rel_path("/style.css?v=2").as_deref(),
            Some("style.css")
        );
        assert_eq!(
            safe_rel_path("/module/x.html#anchor").as_deref(),
            Some("module/x.html"),
        );
    }

    #[test]
    fn rejects_traversal() {
        assert_eq!(safe_rel_path("/../Cargo.toml"), None);
        assert_eq!(safe_rel_path("/module/../../secret"), None);
        // A normal nested path is allowed.
        assert_eq!(
            safe_rel_path("/module/pseudoscript.html").as_deref(),
            Some("module/pseudoscript.html"),
        );
    }

    #[test]
    fn content_type_by_extension() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("style.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("app.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("logo.svg"), "image/svg+xml");
        assert_eq!(content_type("data.bin"), "application/octet-stream");
    }

    #[test]
    fn livereload_injects_before_body_close() {
        let out = super::inject_livereload(b"<html><body>hi</body></html>".to_vec());
        let html = String::from_utf8(out).unwrap();
        assert!(html.contains("__livereload"));
        // injected inside the body, before its close
        assert!(html.find("__livereload").unwrap() < html.find("</body>").unwrap());
    }

    #[test]
    fn livereload_appends_when_no_body() {
        let out = super::inject_livereload(b"<svg></svg>".to_vec());
        let html = String::from_utf8(out).unwrap();
        assert!(html.starts_with("<svg></svg>"));
        assert!(html.contains("__livereload"));
    }

    #[test]
    fn source_change_detection() {
        let out = Path::new("/proj/target/doc");
        // a model source outside the out dir triggers a rebuild
        assert!(super::is_source_change(
            Path::new("/proj/banking/core.pds"),
            out
        ));
        assert!(super::is_source_change(Path::new("/proj/pds.toml"), out));
        // generated output and unrelated files do not
        assert!(!super::is_source_change(
            Path::new("/proj/target/doc/index.html"),
            out
        ));
        assert!(!super::is_source_change(Path::new("/proj/notes.md"), out));
    }
}
