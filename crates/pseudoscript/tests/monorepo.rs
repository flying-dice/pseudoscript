//! Integration tests for local path dependencies and monorepo orchestration
//! (`LANG.md` §8.3, ADR-026): `pds check` across a local `path` dependency,
//! `pds list` discovery, and `pds check --all` aggregation.

use std::path::Path;

use assert_cmd::Command as AssertCommand;

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

/// Lays out a monorepo with a `shared` workspace and a `consumer` workspace that
/// declares `shared` as a local path dependency. Returns the repo temp dir.
fn monorepo_with_local_dep(consumer_main: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write(&root.join("shared/pds.toml"), "[doc]\nname = \"shared\"\n");
    write(
        &root.join("shared/types.pds"),
        "//! shared types\n\npublic system Ledger;\n",
    );

    write(
        &root.join("consumer/pds.toml"),
        "[doc]\nname = \"consumer\"\n\n[dependencies]\nshared = { path = \"../shared\" }\n",
    );
    write(&root.join("consumer/main.pds"), consumer_main);
    tmp
}

#[test]
fn check_resolves_public_node_via_local_path_dep() {
    let tmp =
        monorepo_with_local_dep("//! app\n\npublic container App for shared::types::Ledger;\n");
    pds(&tmp.path().join("consumer"))
        .arg("check")
        .arg(tmp.path().join("consumer"))
        .assert()
        .success();
}

#[test]
fn check_rejects_reference_to_private_node_in_local_dep() {
    // Make the shared node private; the cross-workspace reference must fail.
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(&root.join("shared/pds.toml"), "[doc]\nname = \"shared\"\n");
    write(
        &root.join("shared/types.pds"),
        "//! shared\n\nsystem Ledger;\n",
    );
    write(
        &root.join("consumer/pds.toml"),
        "[doc]\nname = \"consumer\"\n\n[dependencies]\nshared = { path = \"../shared\" }\n",
    );
    write(
        &root.join("consumer/main.pds"),
        "//! app\n\npublic container App for shared::types::Ledger;\n",
    );

    pds(&root.join("consumer"))
        .arg("check")
        .arg(root.join("consumer"))
        .assert()
        .failure();
}

#[test]
fn no_source_dependency_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let consumer = tmp.path();
    write(
        &consumer.join("pds.toml"),
        "[doc]\nname = \"app\"\n\n[dependencies]\nbad = { tag = \"v1\" }\n",
    );
    write(
        &consumer.join("main.pds"),
        "//! app\n\npublic system App;\n",
    );

    pds(consumer).arg("check").assert().failure();
}

#[test]
fn list_enumerates_workspaces_under_root() {
    let tmp = monorepo_with_local_dep("//! app\n\npublic system App;\n");
    let output = pds(tmp.path()).arg("list").output().expect("run pds list");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("consumer"), "{stdout}");
    assert!(stdout.contains("shared"), "{stdout}");
}

#[test]
fn check_all_passes_when_every_workspace_is_well_formed() {
    let tmp =
        monorepo_with_local_dep("//! app\n\npublic container App for shared::types::Ledger;\n");
    pds(tmp.path()).args(["check", "--all"]).assert().success();
}

#[test]
fn check_all_fails_when_any_workspace_fails() {
    // The consumer references a node that does not exist in shared.
    let tmp =
        monorepo_with_local_dep("//! app\n\npublic container App for shared::types::Missing;\n");
    pds(tmp.path()).args(["check", "--all"]).assert().failure();
}
