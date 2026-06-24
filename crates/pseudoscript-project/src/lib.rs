//! The native filesystem edge for the `pds` toolchain (`LANG.md` §8.1, §8.3).
//!
//! Pure analysis lives in [`pseudoscript_model`], which never touches disk.
//! This crate is where the native tools (the `pds` binary, the stdio LSP) read
//! the project: it finds the workspace root (`pds.toml`), walks the tree for
//! `*.pds` modules — skipping `target/` and the vendored `pds_modules/` — and
//! loads each direct dependency's modules for cross-workspace resolution. The
//! slug computation and `pds.lock`/`pds.toml` parsing are shared, not forked:
//! they live in [`pseudoscript_model::deps`].

mod deps;
mod fs;

/// The project-root manifest filename (`LANG.md` §8.1).
pub const MANIFEST: &str = "pds.toml";
/// The lockfile filename (`LANG.md` §8.4).
pub const LOCKFILE: &str = "pds.lock";
/// The project-local directory dependency packages are materialised into.
pub const VENDOR_DIR: &str = "pds_modules";

pub use deps::{dependency_modules, resolve_local, workspace_manifest};
pub use fs::{
    LoadedModule, find_root, is_pds_file, is_visible, load_modules, load_modules_with_paths,
    module_fqn, normalize_segment,
};
