//! Assembles the `pds lang` reference bundle at build time.
//!
//! Concatenates the spec (`LANG.md`), the patterns guide (`PATTERNS.md`), and
//! every file in the `CONFORMANCE/` grammar suite into one blob under
//! `$OUT_DIR/lang-bundle.md`, which `main.rs` embeds via `include_str!`. Each
//! file is fenced with a `===== <repo-relative path> =====` header so the
//! single stream stays unambiguously splittable.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves from the crate manifest dir");

    // Spec and patterns first, then every conformance file sorted by path so a
    // case sits next to its expected-output pair (e.g. `*.pds` then `*.tokens`).
    let mut files = vec![repo_root.join("LANG.md"), repo_root.join("PATTERNS.md")];
    collect_files(&repo_root.join("CONFORMANCE"), &mut files);
    files[2..].sort();

    let mut bundle = String::new();
    for path in &files {
        let rel = path
            .strip_prefix(&repo_root)
            .expect("every source lives under the repo root");
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("reading `{}`: {err}", path.display()));
        write!(bundle, "\n\n===== {} =====\n\n", rel.display())
            .expect("writing to a String never fails");
        bundle.push_str(&contents);
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("lang-bundle.md");
    fs::write(&out, bundle).unwrap_or_else(|err| panic!("writing `{}`: {err}", out.display()));
}

/// Appends every regular file under `dir`, recursively, to `files`.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("reading `{}`: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("conformance dir entry").path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}
