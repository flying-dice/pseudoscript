//! Integration tests for `pds eval`: read a model from stdin and report
//! diagnostics, exiting non-zero on any error. The stdin path is what lets an
//! agent check a snippet without writing a file.

use assert_cmd::Command as AssertCommand;

/// A built `pds eval` command, ready for `.write_stdin(..)`.
fn pds_eval() -> AssertCommand {
    let mut cmd = AssertCommand::cargo_bin("pds").expect("pds binary built");
    cmd.arg("eval");
    cmd
}

#[test]
fn eval_accepts_a_well_formed_model() {
    pds_eval()
        .write_stdin("//! m\n\npublic system AcmeTickets;\n")
        .assert()
        .success();
}

#[test]
fn eval_reports_argument_type_mismatch_and_exits_nonzero() {
    let src = "//! m\n\
\n\
public system S;\n\
public data Money;\n\
\n\
public container C for S {\n\
  run(): void { charge(\"free\") }\n\
  charge(amt: Money): void { }\n\
}\n";
    let output = pds_eval().write_stdin(src).output().expect("run pds eval");
    assert!(
        !output.status.success(),
        "expected non-zero exit on a type error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("argument 1: expected `Money`, found `string`"),
        "diagnostic not found in stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("<stdin>:"),
        "diagnostics should be labelled <stdin>:\n{stderr}"
    );
}
