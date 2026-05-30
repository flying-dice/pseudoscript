//! BDD harness for the LSP crate: drives the pure analysis functions over
//! sources (including `CONFORMANCE/static/` fixtures) and runs a stdio smoke
//! test against the in-process server.

use std::path::{Path, PathBuf};

use cucumber::{World, given, then, when};
use pseudoscript_lsp::analysis;
use pseudoscript_model::Workspace;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, TextEdit};

/// Builds a single-module workspace from `src`, deriving the FQN from its `//!`
/// inner doc (empty when absent), as the server does for a standalone file.
fn single_module(src: &str) -> (Workspace, String) {
    let ast = pseudoscript_syntax::parse(src).ast;
    let fqn = ast
        .inner_docs
        .first()
        .and_then(|doc| doc.text.split_whitespace().next())
        .unwrap_or("")
        .to_owned();
    (Workspace::build(std::iter::once((fqn.clone(), ast))), fqn)
}

/// Absolute path to the workspace `CONFORMANCE/` directory.
fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../CONFORMANCE")
        .canonicalize()
        .expect("CONFORMANCE directory exists")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(conformance_dir().join("static").join(name)).expect("readable fixture")
}

#[derive(Debug, Default, World)]
struct LspWorld {
    src: String,
    diagnostics: Vec<Diagnostic>,
    edit: Option<TextEdit>,
    initialize_response: String,
    semantic: Vec<SemTok>,
}

/// A decoded semantic token in absolute coordinates, for assertion.
#[derive(Debug, Clone)]
struct SemTok {
    line: u32,
    start: u32,
    ty: String,
    declared: bool,
}

// --- diagnostics ------------------------------------------------------------

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[given(regex = r#"^the source fixture "(.+)"$"#)]
fn given_fixture(world: &mut LspWorld, name: String) {
    world.src = fixture(&name);
}

#[given(regex = r"^the inline source:$")]
fn given_inline(world: &mut LspWorld, step: &cucumber::gherkin::Step) {
    world
        .src
        .clone_from(step.docstring().expect("docstring source"));
}

#[when("I compute LSP diagnostics")]
fn compute_diagnostics(world: &mut LspWorld) {
    world.diagnostics = analysis::diagnostics(&world.src);
}

#[then(regex = r"^there are (\d+) diagnostics$")]
fn diagnostic_count(world: &mut LspWorld, n: usize) {
    assert_eq!(
        world.diagnostics.len(),
        n,
        "diagnostics: {:?}",
        world.diagnostics
    );
}

#[then("every diagnostic has error severity")]
fn all_errors(world: &mut LspWorld) {
    assert!(
        world
            .diagnostics
            .iter()
            .all(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
        "diagnostics: {:?}",
        world.diagnostics
    );
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^a diagnostic message contains "(.+)"$"#)]
fn message_contains(world: &mut LspWorld, needle: String) {
    assert!(
        world
            .diagnostics
            .iter()
            .any(|d| d.message.contains(&needle)),
        "no diagnostic message contained {needle:?}; got {:?}",
        world.diagnostics
    );
}

#[then(regex = r"^a diagnostic starts on 0-based line (\d+)$")]
fn diagnostic_on_line(world: &mut LspWorld, line: u32) {
    assert!(
        world.diagnostics.iter().any(|d| d.range.start.line == line),
        "no diagnostic started on line {line}; got {:?}",
        world
            .diagnostics
            .iter()
            .map(|d| d.range.start.line)
            .collect::<Vec<_>>()
    );
}

// --- formatting -------------------------------------------------------------

#[when("I compute a formatting edit")]
fn compute_edit(world: &mut LspWorld) {
    world.edit = analysis::format_edit(&world.src);
}

#[then("there is a formatting edit")]
fn has_edit(world: &mut LspWorld) {
    assert!(world.edit.is_some(), "expected a formatting edit");
}

#[then("there is no formatting edit")]
fn no_edit(world: &mut LspWorld) {
    assert!(
        world.edit.is_none(),
        "expected no edit, got {:?}",
        world.edit
    );
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the edit text is "(.*)"$"#)]
fn edit_text(world: &mut LspWorld, expected: String) {
    let expected = expected.replace("\\n", "\n");
    let edit = world.edit.as_ref().expect("an edit");
    assert_eq!(edit.new_text, expected);
}

// --- hover / definition -----------------------------------------------------

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^hovering the first "(.+)" mentions "(.+)"$"#)]
fn hover_mentions(world: &mut LspWorld, ident: String, needle: String) {
    let offset = world.src.find(&ident).expect("ident present") as u32 + 1;
    let pos = byte_offset_to_position(&world.src, offset);
    let (ws, fqn) = single_module(&world.src);
    let hover = analysis::hover(&ws, &fqn, &world.src, pos).expect("hover present");
    let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(
        m.value.contains(&needle),
        "hover {:?} did not contain {needle:?}",
        m.value
    );
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^go-to-definition on the second "(.+)" resolves$"#)]
fn definition_resolves(world: &mut LspWorld, ident: String) {
    let first = world.src.find(&ident).expect("first occurrence");
    let second = world.src[first + 1..]
        .find(&ident)
        .map(|o| first + 1 + o)
        .expect("second occurrence");
    let pos = byte_offset_to_position(&world.src, second as u32 + 1);
    let (ws, fqn) = single_module(&world.src);
    assert!(
        analysis::definition(&ws, &fqn, &world.src, pos).is_some(),
        "definition did not resolve"
    );
}

// --- semantic tokens --------------------------------------------------------

#[when("I compute semantic tokens")]
fn compute_semantic(world: &mut LspWorld) {
    let legend = analysis::semantic_legend();
    let tokens = analysis::semantic_tokens(&world.src);
    let mut line = 0;
    let mut start = 0;
    world.semantic = tokens
        .data
        .iter()
        .map(|t| {
            if t.delta_line == 0 {
                start += t.delta_start;
            } else {
                line += t.delta_line;
                start = t.delta_start;
            }
            SemTok {
                line,
                start,
                ty: legend.token_types[t.token_type as usize]
                    .as_str()
                    .to_owned(),
                declared: t.token_modifiers_bitset & 1 != 0,
            }
        })
        .collect();
}

/// The decoded token starting at the first occurrence of `needle`.
fn token_at<'a>(world: &'a LspWorld, needle: &str) -> &'a SemTok {
    let offset = world.src.find(needle).expect("needle present") as u32;
    let pos = byte_offset_to_position(&world.src, offset);
    world
        .semantic
        .iter()
        .find(|t| t.line == pos.line && t.start == pos.character)
        .unwrap_or_else(|| panic!("no semantic token at {needle:?}; got {:?}", world.semantic))
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the "(.+)" token has type "(.+)"$"#)]
fn token_has_type(world: &mut LspWorld, needle: String, ty: String) {
    assert_eq!(token_at(world, &needle).ty, ty, "token {needle:?}");
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the "(.+)" token is declared$"#)]
fn token_is_declared(world: &mut LspWorld, needle: String) {
    assert!(token_at(world, &needle).declared, "token {needle:?}");
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^some token has type "(.+)"$"#)]
fn some_token_has_type(world: &mut LspWorld, ty: String) {
    assert!(
        world.semantic.iter().any(|t| t.ty == ty),
        "no token of type {ty:?}; got {:?}",
        world.semantic
    );
}

/// Minimal byte-offset → LSP position for tests (ASCII fixtures only).
fn byte_offset_to_position(src: &str, offset: u32) -> Position {
    let off = offset as usize;
    let line = src[..off].matches('\n').count() as u32;
    let line_start = src[..off].rfind('\n').map_or(0, |n| n + 1);
    Position::new(line, (off - line_start) as u32)
}

// --- stdio smoke ------------------------------------------------------------

#[when("I drive an in-process initialize handshake")]
fn drive_initialize(world: &mut LspWorld) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    world.initialize_response = runtime.block_on(in_process_initialize());
}

// cucumber binds regex capture groups by value as owned `String`s; the step
// signature is fixed by the attribute macro.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the initialize response advertises "(.+)"$"#)]
fn response_advertises(world: &mut LspWorld, needle: String) {
    assert!(
        world.initialize_response.contains(&needle),
        "initialize response did not contain {needle:?}: {}",
        world.initialize_response
    );
}

/// Spawns the tower-lsp server in-process over an in-memory duplex pipe, sends a
/// framed `initialize` request (then `initialized`/`shutdown`/`exit` so it
/// stops), and returns the raw response bytes as a string. A timeout guards
/// against the read ever hanging.
async fn in_process_initialize() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower_lsp::{LspService, Server};

    async fn send<W: tokio::io::AsyncWrite + Unpin>(w: &mut W, msg: &str) {
        let frame = format!("Content-Length: {}\r\n\r\n{msg}", msg.len());
        w.write_all(frame.as_bytes()).await.expect("write frame");
        w.flush().await.expect("flush");
    }

    let (client_end, server_end) = tokio::io::duplex(8192);
    let (server_read, server_write) = tokio::io::split(server_end);
    let (mut client_read, mut client_write) = tokio::io::split(client_end);

    let server = tokio::spawn(async move {
        let (service, socket) = LspService::new(pseudoscript_lsp::Backend::new);
        Server::new(server_read, server_write, socket)
            .serve(service)
            .await;
    });

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
    // Let `initialize` be answered before `exit` closes the stream, else
    // tower-lsp cancels the in-flight request.
    send(&mut client_write, init).await;
    send(&mut client_write, initialized).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    send(&mut client_write, shutdown).await;
    send(&mut client_write, exit).await;
    drop(client_write);

    let mut buf = Vec::new();
    let read = async {
        let mut chunk = [0u8; 4096];
        loop {
            match client_read.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
    };
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), read).await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    String::from_utf8_lossy(&buf).into_owned()
}

fn main() {
    futures::executor::block_on(LspWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
