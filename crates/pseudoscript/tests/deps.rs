//! Integration tests for `pds add` / `pds install` / `pds update` / `pds remove`
//! against a real local git repository fixture (`LANG.md` §8.4, §8.5, ADR-024).
//!
//! These drive the built `pds` binary end to end over a `file://` git remote, so
//! the fetch, sparse subtree checkout, commit pinning, lockfile, and HEAD
//! verification are exercised against actual git — not mocked.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use tempfile::TempDir;

/// Runs `git` with `args` in `cwd`, asserting success.
fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Writes `contents` to `path`, creating parent directories.
fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, contents).expect("write file");
}

/// A built `pds` command rooted at `dir`.
fn pds(dir: &Path) -> AssertCommand {
    let mut cmd = AssertCommand::cargo_bin("pds").expect("pds binary built");
    cmd.current_dir(dir);
    cmd
}

/// Initialises a git repo at `dir` with a deterministic identity and a commit.
fn init_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
}

/// Builds a dependency repo with a workspace in `model/` and a decoy sibling
/// subdir, returning the repo dir and its HEAD commit. The temp dir is returned
/// to keep the fixture alive for the test's duration.
fn dependency_repo() -> (TempDir, PathBuf, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path().to_path_buf();
    init_repo(&repo);

    // The dependency workspace lives in model/.
    write(&repo.join("model/pds.toml"), "[doc]\nname = \"acme\"\n");
    write(
        &repo.join("model/core.pds"),
        "//! core\n\npublic system Acme;\n",
    );
    // A decoy sibling that MUST NOT be materialised by a sparse subtree fetch.
    write(
        &repo.join("unrelated/big.pds"),
        "//! decoy\nsystem Decoy;\n",
    );

    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    (tmp, repo, head)
}

/// A `file://` URL for a local repo path.
fn file_url(repo: &Path) -> String {
    format!("file://{}", repo.display())
}

/// Reads the consumer's `pds.lock` text.
fn read_lock(consumer: &Path) -> String {
    std::fs::read_to_string(consumer.join("pds.lock")).expect("read pds.lock")
}

/// Sets up a consumer workspace declaring `acme` at `repo`'s `model/` subdir,
/// and runs `pds add`. Returns the consumer dir, the kept fixture temp dir, and
/// the dependency HEAD.
fn consumer_with_added_dep() -> (TempDir, TempDir, String) {
    let (dep_tmp, repo, head) = dependency_repo();
    let consumer_tmp = tempfile::tempdir().expect("tempdir");
    let consumer = consumer_tmp.path();
    write(&consumer.join("pds.toml"), "[doc]\nname = \"app\"\n");
    write(
        &consumer.join("main.pds"),
        "//! app\n\npublic container App for acme::core::Acme;\n",
    );

    pds(consumer)
        .args(["add", &file_url(&repo), "--path", "model", "--name", "acme"])
        .assert()
        .success();

    (consumer_tmp, dep_tmp, head)
}

#[test]
fn add_writes_manifest_and_lock_and_materialises_only_subtree() {
    let (consumer_tmp, _dep, head) = consumer_with_added_dep();
    let consumer = consumer_tmp.path();

    // Manifest gained a git + path entry.
    let manifest = std::fs::read_to_string(consumer.join("pds.toml")).unwrap();
    assert!(manifest.contains("[dependencies]"), "{manifest}");
    assert!(manifest.contains("acme"), "{manifest}");
    assert!(manifest.contains("path = \"model\""), "{manifest}");

    // Lock pins the resolved 40-char commit.
    let lock = read_lock(consumer);
    assert!(lock.contains(&head), "lock missing HEAD {head}:\n{lock}");
    assert_eq!(head.len(), 40, "git HEAD is a full sha");

    // The sparse subtree fetch materialised model/ but not the decoy sibling.
    let vendor = consumer.join("pds_modules");
    let slug = std::fs::read_dir(&vendor)
        .expect("pds_modules exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n != ".fetch"))
        .expect("a vendored package dir");
    assert!(slug.join("model/pds.toml").is_file(), "model/ materialised");
    assert!(
        !slug.join("unrelated").exists(),
        "decoy sibling must not be materialised"
    );

    // The checkout's HEAD equals the locked commit (detached, pinned).
    let checkout_head = git(&slug, &["rev-parse", "HEAD"]);
    assert_eq!(checkout_head, head);

    // gitignore covers the vendor dir.
    let gitignore = std::fs::read_to_string(consumer.join(".gitignore")).unwrap();
    assert!(gitignore.contains("pds_modules"), "{gitignore}");
}

#[test]
fn check_resolves_cross_workspace_dependency() {
    let (consumer_tmp, _dep, _head) = consumer_with_added_dep();
    // The model references acme::core::Acme (public), so checking succeeds. The
    // workspace dir is passed explicitly so the result does not depend on how
    // the spawned process resolves its working directory.
    pds(consumer_tmp.path())
        .arg("check")
        .arg(consumer_tmp.path())
        .assert()
        .success();
}

#[test]
fn install_reproduces_from_lock_and_is_idempotent() {
    let (consumer_tmp, _dep, head) = consumer_with_added_dep();
    let consumer = consumer_tmp.path();

    // A fresh checkout: same manifest + lock, but no pds_modules.
    std::fs::remove_dir_all(consumer.join("pds_modules")).unwrap();
    pds(consumer).arg("install").assert().success();

    let vendor = consumer.join("pds_modules");
    let slug = std::fs::read_dir(&vendor)
        .expect("pds_modules exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n != ".fetch"))
        .expect("a vendored package dir");
    assert_eq!(git(&slug, &["rev-parse", "HEAD"]), head);

    // A second install is a no-op and still succeeds.
    pds(consumer).arg("install").assert().success();
}

#[test]
fn install_refetches_when_checkout_head_drifts() {
    let (consumer_tmp, _dep, head) = consumer_with_added_dep();
    let consumer = consumer_tmp.path();

    let vendor = consumer.join("pds_modules");
    let slug = std::fs::read_dir(&vendor)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n != ".fetch"))
        .expect("a vendored package dir");

    // Tamper: reset the checkout to an empty commit so HEAD != locked rev.
    git(&slug, &["commit", "-q", "--allow-empty", "-m", "tamper"]);
    let tampered = git(&slug, &["rev-parse", "HEAD"]);
    assert_ne!(tampered, head);

    // install must detect the drift, re-fetch, and restore the locked HEAD.
    pds(consumer).arg("install").assert().success();
    assert_eq!(git(&slug, &["rev-parse", "HEAD"]), head);
}

#[test]
fn remove_drops_entry_and_rewrites_lock() {
    let (consumer_tmp, _dep, _head) = consumer_with_added_dep();
    let consumer = consumer_tmp.path();

    pds(consumer).args(["remove", "acme"]).assert().success();

    let manifest = std::fs::read_to_string(consumer.join("pds.toml")).unwrap();
    assert!(!manifest.contains("acme"), "entry removed:\n{manifest}");
    let lock = read_lock(consumer);
    assert!(!lock.contains("acme"), "lock no longer names acme:\n{lock}");
}

#[test]
fn add_fails_on_bad_subpath_and_writes_no_entry() {
    let (_dep_tmp, repo, _head) = dependency_repo();
    let consumer_tmp = tempfile::tempdir().expect("tempdir");
    let consumer = consumer_tmp.path();
    write(&consumer.join("pds.toml"), "[doc]\nname = \"app\"\n");

    pds(consumer)
        .args([
            "add",
            &file_url(&repo),
            "--path",
            "does-not-exist",
            "--name",
            "acme",
        ])
        .assert()
        .failure();

    // A failed add writes no [dependencies] entry.
    let manifest = std::fs::read_to_string(consumer.join("pds.toml")).unwrap();
    assert!(!manifest.contains("[dependencies]"), "{manifest}");
}
