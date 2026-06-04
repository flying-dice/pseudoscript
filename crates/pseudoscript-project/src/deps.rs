//! Reading direct-dependency modules off disk (`LANG.md` §8.3, ADR-026).
//!
//! Git dependencies are materialised under `pds_modules/` (located via
//! `pds.lock`); local dependencies are read live from a sibling workspace. The
//! slug/lock/prefix logic is shared with the browser via
//! [`pseudoscript_model::deps`]; this module only adds the filesystem walk.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pseudoscript_model::WorkspaceModule;
use pseudoscript_model::deps::{
    DepFile, Source, parse_dependencies, parse_lock, resolve_dependency_modules,
};

use crate::fs::load_modules;
use crate::{LOCKFILE, MANIFEST, VENDOR_DIR};

/// Loads the modules of every *direct* dependency (`LANG.md` §8.3) for
/// cross-workspace resolution, each module's FQN prefixed with the dependency
/// name (so the consumer addresses `dep::module::Node`). Returns an empty list
/// when the workspace declares no dependencies.
///
/// Git dependencies are read from `pds_modules/` (via `pds.lock`); local
/// dependencies are read live from disk (ADR-026). Only direct dependencies are
/// loaded; transitive packages are not addressable from the consumer.
///
/// # Errors
///
/// Returns an error if the lockfile or manifest is unreadable, a git dependency
/// is not installed under `pds_modules/`, or a local dependency's path is not a
/// workspace.
pub fn dependency_modules(root: &Path) -> Result<Vec<WorkspaceModule>> {
    let lock_path = root.join(LOCKFILE);
    let lock_toml = if lock_path.is_file() {
        std::fs::read_to_string(&lock_path)
            .with_context(|| format!("reading `{}`", lock_path.display()))?
    } else {
        String::new()
    };

    let vendored = collect_vendored(root, &lock_toml)?;
    let local = collect_local(root)?;
    resolve_dependency_modules(&lock_toml, &vendored, &local)
}

/// Reads each git dependency's package files off `pds_modules/` as flat
/// `(slug, file)` pairs for the resolver. Empty when the workspace has no
/// `pds.lock`.
fn collect_vendored(root: &Path, lock_toml: &str) -> Result<Vec<(String, DepFile)>> {
    if lock_toml.trim().is_empty() {
        return Ok(Vec::new());
    }
    let lock = parse_lock(lock_toml)?;
    let vendor = root.join(VENDOR_DIR);

    let mut vendored = Vec::new();
    for edge in &lock.root {
        let id = edge.id();
        let dir = vendor.join(id.slug());
        if !workspace_manifest(&dir, &id.path).is_file() {
            bail!(
                "dependency `{}` is not installed — run `pds install`",
                edge.name
            );
        }
        let ws_root = if id.path.is_empty() {
            dir
        } else {
            dir.join(&id.path)
        };
        for file in dep_files(&ws_root)? {
            vendored.push((id.slug(), file));
        }
    }
    Ok(vendored)
}

/// Reads each local-source dependency's files live from disk (ADR-026) as flat
/// `(name, file)` pairs.
fn collect_local(root: &Path) -> Result<Vec<(String, DepFile)>> {
    let manifest = root.join(MANIFEST);
    if !manifest.is_file() {
        return Ok(Vec::new());
    }
    let manifest_toml = std::fs::read_to_string(&manifest)
        .with_context(|| format!("reading `{}`", manifest.display()))?;

    let mut local = Vec::new();
    for (name, spec) in parse_dependencies(&manifest_toml)? {
        if let Source::Local { path } = spec.source(&name)? {
            let dir = resolve_local(root, &name, &path)?;
            for file in dep_files(&dir)? {
                local.push((name.clone(), file));
            }
        }
    }
    Ok(local)
}

/// Walks a dependency workspace at `ws_root`, returning its modules as
/// [`DepFile`]s (FQN relative to that root, prefixing is the resolver's job).
fn dep_files(ws_root: &Path) -> Result<Vec<DepFile>> {
    Ok(load_modules(ws_root)?
        .into_iter()
        .map(|m| DepFile::new(m.fqn, m.source))
        .collect())
}

/// The workspace manifest path for a fetched package: `<dir>/<path>/pds.toml`.
/// Shared with the binary's `pds install`/`add` write-path.
#[must_use]
pub fn workspace_manifest(dir: &Path, sub: &str) -> PathBuf {
    if sub.is_empty() {
        dir.join(MANIFEST)
    } else {
        dir.join(sub).join(MANIFEST)
    }
}

/// Resolves a local-source dependency directory relative to the consumer root
/// and confirms it is a workspace (ADR-026). Returns the resolved workspace dir.
/// Shared with the binary's `pds add`/`update` validation.
///
/// # Errors
///
/// Returns an error if the resolved path is not a directory or has no `pds.toml`.
pub fn resolve_local(root: &Path, name: &str, path: &str) -> Result<PathBuf> {
    let dir = root.join(path);
    if !dir.join(MANIFEST).is_file() {
        bail!(
            "local dependency `{name}` at `{}` is not a workspace (no `{MANIFEST}`)",
            dir.display()
        );
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_model::deps::PackageId;

    /// Writes a `pds.lock` with a single `root` edge for `id` named `name`.
    fn write_lock(root: &Path, name: &str, id: &PackageId) {
        let lock = format!(
            "version = 1\n\n[[root]]\nname = \"{name}\"\nsource = \"{}\"\nrev = \"{}\"\npath = \"{}\"\n",
            id.source, id.rev, id.path
        );
        std::fs::write(root.join(LOCKFILE), lock).unwrap();
    }

    #[test]
    fn dependency_modules_loads_direct_deps_prefixed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let id = PackageId {
            source: "https://x/acme/auth".into(),
            rev: "0123456789abcdef".into(),
            path: "login".into(),
        };
        let ws = root.join(VENDOR_DIR).join(id.slug()).join(&id.path);
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(ws.join(MANIFEST), "[doc]\nname = \"auth\"\n").unwrap();
        std::fs::write(ws.join("core.pds"), "//! c\npublic system Login;\n").unwrap();
        write_lock(root, "auth", &id);

        let modules = dependency_modules(root).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].fqn, "auth::core");
        assert!(modules[0].source.contains("public system Login"));
    }

    #[test]
    fn dependency_modules_empty_without_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        assert!(dependency_modules(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn dependency_modules_loads_local_dep_prefixed() {
        let dir = tempfile::tempdir().unwrap();
        // Consumer at <tmp>/app, sibling at <tmp>/shared — `../shared` stays
        // inside the tempdir, so the test never escapes it.
        let root = dir.path().join("app");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(MANIFEST),
            "[dependencies]\nshared = { path = \"../shared\" }\n",
        )
        .unwrap();
        let shared = dir.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join(MANIFEST), "[doc]\nname = \"shared\"\n").unwrap();
        std::fs::write(
            shared.join("money.pds"),
            "//! m\npublic data Money { v: number }\n",
        )
        .unwrap();

        let modules = dependency_modules(&root).unwrap();
        assert!(modules.iter().any(|m| m.fqn == "shared::money"));
    }

    #[test]
    fn dependency_modules_errors_when_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let id = PackageId {
            source: "https://x/acme/auth".into(),
            rev: "0123456789abcdef".into(),
            path: String::new(),
        };
        write_lock(root, "auth", &id);
        let err = dependency_modules(root).unwrap_err().to_string();
        assert!(err.contains("not installed"), "{err}");
    }
}
