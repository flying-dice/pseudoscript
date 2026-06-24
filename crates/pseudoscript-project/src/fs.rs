//! Finding the project root and walking its `*.pds` modules (`LANG.md` §8.1).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pseudoscript_model::WorkspaceModule;
use walkdir::WalkDir;

use crate::MANIFEST;

/// Walks up from `start` (a file or directory) through its ancestors until a
/// directory containing `pds.toml` is found, returning that (absolute) directory.
///
/// `start` is made absolute first (against the current directory, lexically — no
/// disk access), so the ancestor walk climbs the real tree to the filesystem
/// root. A relative path would otherwise bottom out at the empty path, whose
/// `join("pds.toml")` spuriously resolves against the current directory: that is
/// why `pds check main.pds` reported an IO error walking the empty path while
/// `pds check .` worked — the empty root was then handed to the tree walker
/// (issue #67).
///
/// # Errors
///
/// Returns an error if no ancestor of `start` contains a `pds.toml`.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    let absolute =
        std::path::absolute(start).with_context(|| format!("resolving `{}`", start.display()))?;
    // A file's own path is not a search directory; begin at its parent. After
    // absolutising, a file always has a non-empty parent.
    let from = if absolute.is_file() {
        absolute.parent().unwrap_or(&absolute)
    } else {
        &absolute
    };
    for dir in from.ancestors() {
        if dir.join(MANIFEST).is_file() {
            return Ok(dir.to_path_buf());
        }
    }
    bail!(
        "no `{MANIFEST}` found in `{}` or any parent directory",
        from.display()
    );
}

/// A loaded module paired with the workspace-relative path it was read from.
///
/// [`WorkspaceModule`] is pure `(fqn, source)`; the path is dropped there
/// because the model never touches disk. A tool that *does* know the disk layout
/// (`pds check`, the doc build) keeps this pairing so it can report diagnostics
/// against the real file — the FQN does not round-trip to the path, since a
/// hyphenated segment normalises to `_` (ADR-031: `web-ide/x.pds` →
/// `web_ide::x`).
#[derive(Debug)]
pub struct LoadedModule {
    /// The module's path relative to the workspace root (`pkg/svc.pds`).
    pub relative_path: PathBuf,
    /// The parsed `(fqn, source)` module.
    pub module: WorkspaceModule,
}

/// Walks `root` recursively for `*.pds` files (skipping `target/`,
/// `pds_modules/`, and hidden directories), reading each and deriving its FQN
/// from its path relative to `root`. The result is sorted by FQN for
/// determinism.
///
/// # Errors
///
/// Returns an error if the tree cannot be walked or a `.pds` file read.
pub fn load_modules(root: &Path) -> Result<Vec<WorkspaceModule>> {
    Ok(load_modules_with_paths(root)?
        .into_iter()
        .map(|loaded| loaded.module)
        .collect())
}

/// Like [`load_modules`], but keeps each module's workspace-relative path so a
/// caller can attribute diagnostics to the real file. Sorted by FQN, matching
/// [`load_modules`].
///
/// # Errors
///
/// Returns an error if the tree cannot be walked or a `.pds` file read.
pub fn load_modules_with_paths(root: &Path) -> Result<Vec<LoadedModule>> {
    let mut loaded = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_entry(is_visible) {
        let entry = entry.with_context(|| format!("walking `{}`", root.display()))?;
        let path = entry.path();
        if !is_pds_file(path) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("`{}` is outside the project root", path.display()))?;
        let Some(fqn) = module_fqn(relative) else {
            continue;
        };
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("reading `{}`", path.display()))?;
        loaded.push(LoadedModule {
            relative_path: relative.to_path_buf(),
            module: WorkspaceModule::new(fqn, source),
        });
    }
    loaded.sort_by(|a, b| a.module.fqn.cmp(&b.module.fqn));
    Ok(loaded)
}

/// Whether `entry` should be descended into / kept: skips the `target` and
/// `pds_modules` directories (build output and vendored dependencies) and any
/// hidden entry (a name starting with `.`). The walk root itself is always kept.
#[must_use]
pub fn is_visible(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    if name.starts_with('.') {
        return false;
    }
    !(entry.file_type().is_dir() && (name == "target" || name == "pds_modules"))
}

/// Whether `path` is a regular file with a `.pds` extension.
#[must_use]
pub fn is_pds_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "pds")
}

/// Derives a module FQN from a `.pds` path relative to the project root
/// (`LANG.md` §8.1): each path component becomes a `::`-joined segment, with the
/// `.pds` extension stripped from the filename. `banking/core.pds` →
/// `banking::core`; `pseudoscript.pds` → `pseudoscript`.
///
/// A hyphen in any segment normalises to `_` so a kebab-case filename yields a
/// valid identifier root (ADR-031), as Rust maps a `my-crate` package to the
/// `my_crate` identifier: `web-ide/file-tree.pds` → `web_ide::file_tree`.
///
/// Returns `None` when the path has no usable filename stem.
#[must_use]
pub fn module_fqn(relative: &Path) -> Option<String> {
    let mut segments: Vec<String> = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|c| normalize_segment(&c.as_os_str().to_string_lossy()))
        .collect();
    segments.push(normalize_segment(&relative.file_stem()?.to_string_lossy()));
    Some(segments.join("::"))
}

/// Normalises one path segment into an FQN segment: a hyphen becomes `_`
/// (ADR-031), so a kebab-case directory or filename is addressable. Other
/// characters pass through unchanged.
#[must_use]
pub fn normalize_segment(segment: &str) -> String {
    segment.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fqn_root_level_file_is_its_stem() {
        assert_eq!(
            module_fqn(Path::new("pseudoscript.pds")).as_deref(),
            Some("pseudoscript")
        );
    }

    #[test]
    fn fqn_one_directory_joins_with_double_colon() {
        assert_eq!(
            module_fqn(Path::new("banking/core.pds")).as_deref(),
            Some("banking::core")
        );
    }

    #[test]
    fn fqn_nested_directories_join_each_segment() {
        assert_eq!(
            module_fqn(Path::new("banking/internal/ledger.pds")).as_deref(),
            Some("banking::internal::ledger")
        );
    }

    #[test]
    fn fqn_normalises_hyphens_to_underscores_per_segment() {
        // ADR-031: a kebab-case directory or filename maps to an identifier root.
        assert_eq!(
            module_fqn(Path::new("web-ide/file-tree.pds")).as_deref(),
            Some("web_ide::file_tree")
        );
    }

    #[test]
    fn find_root_walks_up_from_a_file_to_its_manifest() {
        // A `.pds` file inside a workspace resolves to that workspace's root,
        // even when the file sits directly under it — the regression behind
        // `pds check <file>` failing where `pds check .` worked (issue #67).
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join(MANIFEST), "[doc]\nname = \"x\"\n").unwrap();
        std::fs::write(root.join("main.pds"), "//! m\npublic system A;\n").unwrap();

        assert_eq!(find_root(&root.join("main.pds")).unwrap(), root);
    }

    #[test]
    fn find_root_walks_up_from_a_nested_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join(MANIFEST), "[doc]\nname = \"x\"\n").unwrap();
        let nested = root.join("pkg");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("svc.pds"), "//! s\npublic system S;\n").unwrap();

        assert_eq!(find_root(&nested.join("svc.pds")).unwrap(), root);
    }

    #[test]
    fn find_root_errors_with_no_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lone.pds"), "//! l\npublic system L;\n").unwrap();

        assert!(find_root(&dir.path().join("lone.pds")).is_err());
    }

    #[test]
    fn load_modules_skips_pds_modules_and_target() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.pds"), "//! app\npublic system A;\n").unwrap();
        for vendored in ["pds_modules/banking-abc123def456/core.pds", "target/x.pds"] {
            let path = dir.path().join(vendored);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "//! x\npublic system Vendored;\n").unwrap();
        }
        let fqns: Vec<_> = load_modules(dir.path())
            .unwrap()
            .into_iter()
            .map(|m| m.fqn)
            .collect();
        assert_eq!(fqns, ["app"]);
        assert!(!fqns.iter().any(|f| f.contains("pds_modules")));
    }

    #[test]
    fn load_modules_with_paths_keeps_the_real_relative_path() {
        // The path must be the on-disk one, not reconstructed from the FQN: a
        // kebab segment normalises to `_` in the FQN (ADR-031) but the file is
        // still `web-ide/file-tree.pds`. A diagnostic must point at the real file
        // (issue #68).
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("web-ide");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("file-tree.pds"), "//! ft\npublic system T;\n").unwrap();

        let loaded = load_modules_with_paths(dir.path()).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].module.fqn, "web_ide::file_tree");
        assert_eq!(
            loaded[0].relative_path,
            Path::new("web-ide").join("file-tree.pds")
        );
    }
}
