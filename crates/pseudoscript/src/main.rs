//! `pds` — the `PseudoScript` command-line driver.
//!
//! Subcommands wrap the workspace libraries:
//!
//! - `pds lsp` — start the language server over stdio ([`pseudoscript_lsp`]).
//! - `pds check <FILE>` — report diagnostics; exit non-zero on any error.
//! - `pds fmt <FILE> [--write]` — canonical formatting.
//! - `pds tokens <FILE>` — the lexical token stream, for debugging.
//! - `pds doc [PATH]` — generate the static documentation site
//!   ([`pseudoscript_doc`]) for the workspace rooted at the nearest `pds.toml`.
//! - `pds init [PATH]` — bootstrap a new workspace (`pds.toml` + a starter module).
//! - `pds upgrade [VERSION]` — replace the binary with a GitHub release.

mod upgrade;
mod workspace;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use pseudoscript_format::{FormatError, format};
use pseudoscript_model::{Diagnostic, check, check_workspace, graph};
use pseudoscript_syntax::{LineIndex, Severity, render_tokens};

/// The `PseudoScript` toolchain.
#[derive(Debug, Parser)]
#[command(name = "pds", version, about = "PseudoScript toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    /// Start the language server over stdio.
    Lsp,
    /// Check a file and report diagnostics (exit non-zero on any error).
    Check {
        /// The `.pds` file to check.
        file: PathBuf,
    },
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
    /// Generate the static documentation site for the workspace.
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
    },
    /// Download and install a release over the running binary.
    Upgrade {
        /// Release to install, e.g. `0.1.0` or `v0.1.0`. Defaults to the latest.
        version: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path, name } => cmd_init(&path, name.as_deref()),
        Command::Lsp => run_lsp(),
        Command::Check { file } => cmd_check(&file),
        Command::Fmt { file, write } => cmd_fmt(&file, write),
        Command::Tokens { file } => cmd_tokens(&file),
        Command::Doc {
            path,
            serve,
            watch,
            port,
        } => cmd_doc(&path, serve, watch, port),
        Command::Upgrade { version } => cmd_upgrade(version),
    }
}

/// `pds upgrade`: download and install a release over the running binary.
fn cmd_upgrade(version: Option<String>) -> ExitCode {
    match upgrade::run(version) {
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

/// `pds check`: print `path:line:col: severity: message` per diagnostic and
/// exit non-zero if any error-severity diagnostic was produced.
fn cmd_check(path: &Path) -> ExitCode {
    let src = match read(path) {
        Ok(src) => src,
        Err(code) => return code,
    };
    let index = LineIndex::new(&src);
    let diagnostics = check(&src);
    let mut had_error = false;
    for diag in &diagnostics {
        let (line, col) = index.line_col(diag.span.start);
        eprintln!(
            "{}:{line}:{col}: {}: {}",
            path.display(),
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

/// `pds doc`: generate the static documentation site for the workspace rooted
/// at the nearest `pds.toml`.
///
/// Model diagnostics are reported to stderr but, like `cargo doc`, never abort
/// generation — a model with warnings still documents. Only I/O and load errors
/// fail the command.
fn cmd_doc(path: &Path, serve: bool, watch: bool, port: u16) -> ExitCode {
    // Watching only makes sense while serving (regenerate, then live-reload).
    let serve = serve || watch;
    match run_doc(path, serve, watch, port) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Generates the site once, then optionally serves it — with a workspace
/// watcher and browser live-reload when `watch` is set.
fn run_doc(path: &Path, serve: bool, watch: bool, port: u16) -> Result<()> {
    let out_dir = build_site(path, true)?;
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
fn build_site(path: &Path, announce: bool) -> Result<PathBuf> {
    let root = workspace::find_root(path)?;
    let project = workspace::load(&root)?;

    report_diagnostics(&check_workspace(&project.modules));

    let site = pseudoscript_doc::render_site(&graph(&project.modules), &project.config);
    for file in &site.files {
        let dest = project.out_dir.join(&file.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating `{}`", parent.display()))?;
        }
        fs::write(&dest, &file.contents)
            .with_context(|| format!("writing `{}`", dest.display()))?;
    }

    copy_logo(&root, &project);

    if announce {
        println!(
            "Generated {} files in {}",
            site.files.len(),
            project.out_dir.display()
        );
    }
    Ok(project.out_dir)
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
        match build_site(path, false) {
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
