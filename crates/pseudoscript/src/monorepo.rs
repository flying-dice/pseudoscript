//! Monorepo orchestration for `pds list` and the `--all` aggregate modes
//! (`LANG.md` §8.1, ADR-026).
//!
//! A repository may hold several `PseudoScript` workspaces side by side, each
//! its own `pds.toml` root. This module discovers those workspaces by walking a
//! root directory, so the CLI can list them and run `check`/`doc` across the
//! whole set. Cross-workspace resolution itself is unchanged: each discovered
//! workspace resolves its own declared dependencies (git or local) through the
//! existing externals path.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

/// The project-root manifest filename (`LANG.md` §8.1).
const MANIFEST: &str = "pds.toml";

/// Discovers every `pds.toml` workspace under `root`, returning each workspace
/// directory sorted for deterministic output.
///
/// The walk skips `target/`, `pds_modules/` (vendored dependencies), and hidden
/// directories — the same exclusions the single-workspace module walker applies
/// (`workspace::load_modules`). A workspace's own subtree is not descended into
/// once its `pds.toml` is found, so nested manifests under a discovered root are
/// treated as part of that root rather than separate workspaces.
///
/// # Errors
///
/// Returns an error if the tree under `root` cannot be walked.
pub fn discover(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(is_visible)
    {
        let entry = entry.with_context(|| format!("walking `{}`", root.display()))?;
        if entry.file_type().is_dir() && entry.path().join(MANIFEST).is_file() {
            found.push(entry.path().to_path_buf());
        }
    }
    // A workspace owns its subtree: drop any manifest nested under an already
    // discovered (shallower) workspace, so it is not reported separately. The
    // walk is depth-first and pre-order, so a parent always precedes its child.
    let mut roots: Vec<PathBuf> = Vec::new();
    for dir in found {
        if !roots.iter().any(|parent| dir.starts_with(parent)) {
            roots.push(dir);
        }
    }
    Ok(roots)
}

/// Whether `entry` should be descended into during discovery: skips `target`
/// and `pds_modules` directories and any hidden entry. The walk root is kept.
fn is_visible(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    if name.starts_with('.') {
        return false;
    }
    !(entry.file_type().is_dir() && (name == "target" || name == "pds_modules"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes an empty `pds.toml` workspace at `dir`.
    fn workspace(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(MANIFEST), "[doc]\n").unwrap();
    }

    #[test]
    fn discovers_sibling_workspaces_skipping_vendor_and_hidden() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        workspace(&root.join("alpha"));
        workspace(&root.join("beta"));
        // Excluded: vendored, build output, hidden, and a nested manifest.
        workspace(&root.join("alpha/pds_modules/dep"));
        workspace(&root.join("beta/target/doc"));
        workspace(&root.join(".hidden/ws"));
        workspace(&root.join("alpha/internal"));

        let found = discover(root).unwrap();
        let names: Vec<_> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["alpha", "beta"]);
    }

    #[test]
    fn discovers_the_root_itself_when_it_is_a_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        workspace(tmp.path());
        let found = discover(tmp.path()).unwrap();
        assert_eq!(found, [tmp.path().to_path_buf()]);
    }
}
