//! BDD harness for the `pds` CLI: drives the built binary against conformance
//! fixtures and asserts exit status and output.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use cucumber::{World, given, then, when};

/// Absolute path to the workspace `CONFORMANCE/` directory.
fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../CONFORMANCE")
        .canonicalize()
        .expect("CONFORMANCE directory exists")
}

/// Absolute path to this crate's `tests/fixtures` directory.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// A fresh `pds` command for the built binary under test.
fn pds() -> Command {
    Command::cargo_bin("pds").expect("pds binary built")
}

#[derive(Debug, Default, World)]
struct CliWorld {
    /// Path the current command operates on (may be a temp copy).
    target: PathBuf,
    /// A scratch directory kept alive for the scenario's temp files.
    scratch: Option<PathBuf>,
    /// The generated site's output directory, for `pds doc` scenarios.
    out_dir: Option<PathBuf>,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Drop for CliWorld {
    fn drop(&mut self) {
        if let Some(dir) = &self.scratch {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

// --- given ------------------------------------------------------------------

#[given(regex = r#"^the conformance fixture "(.+)"$"#)]
fn given_fixture(world: &mut CliWorld, rel: String) {
    world.target = conformance_dir().join(rel);
}

#[given(regex = r#"^a writable copy of fixture "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn given_writable_copy(world: &mut CliWorld, rel: String) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let src = conformance_dir().join(&rel);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pds-cli-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let name = Path::new(&rel).file_name().expect("file name");
    let dest = dir.join(name);
    std::fs::copy(&src, &dest).expect("copy fixture");
    world.scratch = Some(dir);
    world.target = dest;
}

#[given(regex = r#"^a writable copy of fixture workspace "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn given_writable_workspace(world: &mut CliWorld, name: String) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let src = fixtures_dir().join(&name);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pds-doc-{}-{unique}", std::process::id()));
    copy_dir(&src, &dir);
    world.scratch = Some(dir.clone());
    world.target = dir;
}

/// Recursively copies the directory `src` to `dest`.
fn copy_dir(src: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).expect("create dest dir");
    for entry in std::fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let to = dest.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).expect("copy file");
        }
    }
}

// --- when -------------------------------------------------------------------

fn run(world: &mut CliWorld, args: &[&str]) {
    let mut cmd = pds();
    cmd.args(args);
    cmd.arg(&world.target);
    let output = cmd.output().expect("run pds");
    world.exit_code = output.status.code();
    world.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    world.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
}

/// Runs `pds` with `args` and no trailing path target, capturing the result.
fn run_bare(world: &mut CliWorld, args: &[&str]) {
    let output = pds().args(args).output().expect("run pds");
    world.exit_code = output.status.code();
    world.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    world.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
}

#[when("I run pds check")]
fn run_check(world: &mut CliWorld) {
    run(world, &["check"]);
}

#[when("I run pds lang")]
fn run_lang(world: &mut CliWorld) {
    run_bare(world, &["lang"]);
}

#[when("I run pds spec")]
fn run_spec(world: &mut CliWorld) {
    run_bare(world, &["spec"]);
}

#[when("I run pds skill")]
fn run_skill(world: &mut CliWorld) {
    run_bare(world, &["skill"]);
}

#[when("I run pds tokens")]
fn run_tokens(world: &mut CliWorld) {
    run(world, &["tokens"]);
}

#[when("I run pds fmt")]
fn run_fmt(world: &mut CliWorld) {
    run(world, &["fmt"]);
}

#[when("I run pds fmt --write")]
fn run_fmt_write(world: &mut CliWorld) {
    run(world, &["fmt", "--write"]);
}

#[when("I run pds doc on the workspace")]
fn run_doc(world: &mut CliWorld) {
    run(world, &["doc"]);
    world.out_dir = Some(world.target.join("target/doc"));
}

#[when("I run pds outline on the workspace")]
fn run_outline(world: &mut CliWorld) {
    run(world, &["outline"]);
}

#[when("I run pds svg for the context view")]
fn run_svg_context(world: &mut CliWorld) {
    run(world, &["svg", "--view", "context"]);
}

#[when("I run pds svg for an unknown symbol")]
fn run_svg_unknown(world: &mut CliWorld) {
    run(world, &["svg", "--symbol", "nope::Nope"]);
}

// --- then -------------------------------------------------------------------

#[then("the exit code is zero")]
fn exit_zero(world: &mut CliWorld) {
    assert_eq!(world.exit_code, Some(0), "stderr: {}", world.stderr);
}

#[then("the exit code is non-zero")]
fn exit_nonzero(world: &mut CliWorld) {
    assert!(
        world.exit_code.is_some_and(|c| c != 0),
        "expected non-zero exit, got {:?}",
        world.exit_code
    );
}

#[then("stderr is empty")]
fn stderr_empty(world: &mut CliWorld) {
    assert!(world.stderr.is_empty(), "stderr: {}", world.stderr);
}

#[then(regex = r#"^stderr contains "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn stderr_contains(world: &mut CliWorld, needle: String) {
    assert!(
        world.stderr.contains(&needle),
        "stderr {:?} did not contain {needle:?}",
        world.stderr
    );
}

#[then(regex = r#"^stdout contains "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn stdout_contains(world: &mut CliWorld, needle: String) {
    assert!(
        world.stdout.contains(&needle),
        "stdout {:?} did not contain {needle:?}",
        world.stdout
    );
}

#[then(regex = r#"^stdout equals the golden "(.+)"$"#)]
fn stdout_equals_golden(world: &mut CliWorld, rel: String) {
    let golden = std::fs::read_to_string(conformance_dir().join(rel)).expect("read golden");
    assert_eq!(world.stdout, golden);
}

#[then("stdout is canonical and idempotent")]
fn stdout_idempotent(world: &mut CliWorld) {
    // Re-format the formatted output and assert it is unchanged.
    let once = world.stdout.clone();
    let twice = pseudoscript_format::format(&once).expect("formatted output re-formats");
    assert_eq!(once, twice, "formatting is not idempotent");
}

#[then(regex = r#"^the file now equals the golden "(.+)"$"#)]
fn file_equals_golden(world: &mut CliWorld, rel: String) {
    let golden = std::fs::read_to_string(conformance_dir().join(rel)).expect("read golden");
    let actual = std::fs::read_to_string(&world.target).expect("read written file");
    assert_eq!(actual, golden);
}

#[then("the file is unchanged")]
fn file_unchanged(world: &mut CliWorld) {
    // The fixture this is used with is a known-bad source; assert it still has
    // a parse error (i.e. was not rewritten to canonical form).
    let actual = std::fs::read_to_string(&world.target).expect("read file");
    assert!(
        pseudoscript_format::format(&actual).is_err(),
        "file appears to have been formatted despite a parse error"
    );
}

// --- pds doc site assertions ------------------------------------------------

/// The site output directory, set by the `pds doc` `when` step.
fn out_dir(world: &CliWorld) -> &Path {
    world.out_dir.as_deref().expect("doc was run")
}

#[then(regex = r#"^the site has "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn site_has(world: &mut CliWorld, rel: String) {
    let path = out_dir(world).join(&rel);
    assert!(
        path.is_file(),
        "expected site file `{rel}` at `{}`",
        path.display()
    );
}

#[then(regex = r#"^the site has a "(.+)" page$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn site_has_page_under(world: &mut CliWorld, dir: String) {
    let path = out_dir(world).join(&dir);
    let count = std::fs::read_dir(&path).map_or(0, |entries| entries.flatten().count());
    assert!(count > 0, "expected at least one page under `{dir}`");
}

#[then(regex = r#"^the site index contains "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn site_index_contains(world: &mut CliWorld, needle: String) {
    let index = std::fs::read_to_string(out_dir(world).join("index.html")).expect("read index");
    assert!(
        index.contains(&needle),
        "index.html did not contain {needle:?}"
    );
}

// --- pds lsp stdio smoke ----------------------------------------------------

#[when("I run pds lsp and send an initialize handshake")]
fn run_lsp_handshake(world: &mut CliWorld) {
    use std::io::Write;
    use std::process::{Command as Proc, Stdio};

    let bin = assert_cmd::cargo::cargo_bin("pds");
    let mut child = Proc::new(bin)
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pds lsp");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    let write_frame = |stdin: &mut std::process::ChildStdin, msg: &str| {
        let frame = format!("Content-Length: {}\r\n\r\n{msg}", msg.len());
        stdin.write_all(frame.as_bytes()).expect("write frame");
        stdin.flush().expect("flush");
    };

    // Read the `initialize` response on a thread (read exactly one framed
    // message) so a misbehaving server cannot hang the test. We must read the
    // response BEFORE closing stdin: an EOF on stdin makes tower-lsp cancel any
    // in-flight request (`-32800 Canceled`).
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_one_message(&mut stdout));
    });

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
    write_frame(&mut stdin, init);
    write_frame(&mut stdin, initialized);

    world.stdout = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("lsp responded before timeout");

    // Now wind the server down cleanly.
    write_frame(&mut stdin, shutdown);
    write_frame(&mut stdin, exit);
    drop(stdin);
    let _ = child.wait();
}

/// Reads exactly one LSP message (`Content-Length` header + body) from `r`,
/// returning the JSON body. Returns whatever was read on early EOF.
fn read_one_message(r: &mut impl std::io::Read) -> String {
    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    // Read header bytes up to the blank line `\r\n\r\n`.
    while r.read(&mut byte).is_ok_and(|n| n == 1) {
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = String::from_utf8_lossy(&header);
    let len: usize = header
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    String::from_utf8_lossy(&body).into_owned()
}

#[then(regex = r#"^the lsp response advertises "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn lsp_advertises(world: &mut CliWorld, needle: String) {
    assert!(
        world.stdout.contains(&needle),
        "lsp response {:?} did not contain {needle:?}",
        world.stdout
    );
}

// --- init -------------------------------------------------------------------

#[given("an empty workspace directory")]
fn given_empty_dir(world: &mut CliWorld) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pds-init-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    world.scratch = Some(dir.clone());
    world.target = dir;
}

#[when("I run pds init")]
fn run_init(world: &mut CliWorld) {
    run(world, &["init"]);
}

#[when("I run pds check on the generated module")]
fn run_check_generated(world: &mut CliWorld) {
    let main = world.target.join("main.pds");
    let output = pds().arg("check").arg(&main).output().expect("run pds");
    world.exit_code = output.status.code();
    world.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    world.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
}

#[then(regex = r#"^the workspace contains "(.+)"$"#)]
// cucumber parses each regex capture into an owned `String`; `&str` won't compile here.
#[allow(clippy::needless_pass_by_value)]
fn workspace_contains(world: &mut CliWorld, rel: String) {
    let path = world.target.join(&rel);
    assert!(path.exists(), "expected {} to exist", path.display());
}

fn main() {
    futures::executor::block_on(CliWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
