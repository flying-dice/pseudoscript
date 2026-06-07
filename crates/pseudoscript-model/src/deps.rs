//! The pure read-path for git/local dependencies (`LANG.md` §8.3, §8.4,
//! ADR-024, ADR-026).
//!
//! This is the WASM-safe core shared by the native loader
//! (`pseudoscript-project`, which adds the filesystem walk + git fetch) and the
//! browser bridge (`pseudoscript-ide`, which feeds in file bytes read through
//! the File System Access API). It operates only on strings and in-memory file
//! sets — it never touches the filesystem, runs git, or hashes for the temp
//! cache. The slug computation and `pds.lock`/`pds.toml` parsing live here once
//! so neither edge forks them.
//!
//! A package's identity is `(source, rev, path)` (§8.4): the same repo at
//! different revisions or sub-paths are distinct packages.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::WorkspaceModule;

// ---------------------------------------------------------------------------
// Manifest `[dependencies]` model
// ---------------------------------------------------------------------------

/// A `[dependencies]` entry as written in `pds.toml` (`LANG.md` §8.3).
///
/// The source is selected by `git` (ADR-024, ADR-026): present → a git source,
/// absent → a local source whose `path` names a sibling workspace. `path` is
/// overloaded — under git it is the in-repo subdirectory, under local it is the
/// manifest-relative directory.
#[derive(Debug, Clone, Deserialize)]
pub struct DepSpec {
    /// The git source URL, or `None` for a local source.
    pub git: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub branch: Option<String>,
    /// Under a git source: the dependency workspace's directory within the repo
    /// (default = root). Under a local source: the sibling workspace's path,
    /// relative to the declaring manifest.
    pub path: Option<String>,
}

/// The resolved source of a `[dependencies]` entry (ADR-026): a fetched git
/// repository, or a local sibling workspace read live from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A git source: a remote URL, a revision selector, and an in-repo subdir.
    Git {
        url: String,
        selector: Rev,
        sub: String,
    },
    /// A local source: a manifest-relative path to a sibling workspace.
    Local { path: String },
}

/// A resolved revision selector: at most one of tag/rev/branch (§8.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rev {
    Tag(String),
    Branch(String),
    Commit(String),
    /// No selector — the remote's default-branch HEAD.
    Default,
}

impl Rev {
    /// Builds a selector from the three mutually-exclusive CLI flags.
    ///
    /// # Errors
    ///
    /// Returns an error if more than one of `tag`/`rev`/`branch` is set.
    pub fn from_flags(
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
    ) -> Result<Self> {
        match (tag, rev, branch) {
            (None, None, None) => Ok(Self::Default),
            (Some(t), None, None) => Ok(Self::Tag(t)),
            (None, Some(r), None) => Ok(Self::Commit(r)),
            (None, None, Some(b)) => Ok(Self::Branch(b)),
            _ => bail!("set at most one of --tag, --rev, --branch"),
        }
    }

    /// The `(tag, rev, branch)` flags this selector corresponds to — the
    /// inverse of [`Rev::from_flags`].
    #[must_use]
    pub fn to_flags(&self) -> (Option<String>, Option<String>, Option<String>) {
        match self {
            Self::Tag(t) => (Some(t.clone()), None, None),
            Self::Commit(r) => (None, Some(r.clone()), None),
            Self::Branch(b) => (None, None, Some(b.clone())),
            Self::Default => (None, None, None),
        }
    }

    /// A stable key distinguishing selectors, for the fetch cache.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Tag(t) => format!("tag:{t}"),
            Self::Branch(b) => format!("branch:{b}"),
            Self::Commit(c) => format!("rev:{c}"),
            Self::Default => "default".to_owned(),
        }
    }
}

impl DepSpec {
    /// The dependency's resolved source (ADR-024, ADR-026).
    ///
    /// # Errors
    ///
    /// Returns an error if the entry declares no source (neither `git` nor
    /// `path`), or if a git source sets more than one revision selector, or if
    /// `path` escapes its base via `..` or an absolute component.
    pub fn source(&self, name: &str) -> Result<Source> {
        // A git source is selected by the presence of `git`; otherwise the entry
        // declares a local source named by `path` (ADR-026).
        let Some(url) = &self.git else {
            return self.local_source(name);
        };
        let selector = Rev::from_flags(self.tag.clone(), self.rev.clone(), self.branch.clone())
            .with_context(|| format!("dependency `{name}`"))?;
        let sub = sanitize_rel_path(self.path.as_deref().unwrap_or(""))
            .with_context(|| format!("dependency `{name}` `path`"))?;
        Ok(Source::Git {
            url: url.clone(),
            selector,
            sub,
        })
    }

    /// The local source of an entry with no `git` (ADR-026): `path` names a
    /// sibling workspace and no revision selector is allowed.
    ///
    /// A local `path` is manifest-relative and `..` is the normal way to reach a
    /// sibling (`path = "../shared"`), so traversal is permitted — only an
    /// absolute path is rejected, since it would break repo portability.
    fn local_source(&self, name: &str) -> Result<Source> {
        let Some(path) = self.path.as_deref() else {
            bail!("dependency `{name}` declares no source: set `git` or `path`");
        };
        if self.tag.is_some() || self.rev.is_some() || self.branch.is_some() {
            bail!("dependency `{name}`: a local `path` source takes no `tag`/`rev`/`branch`");
        }
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            bail!("dependency `{name}`: local `path` is empty");
        }
        if Path::new(path).is_absolute() {
            bail!("dependency `{name}`: local `path` `{path}` must be relative to the manifest");
        }
        Ok(Source::Local {
            path: path.to_owned(),
        })
    }
}

/// A minimal view of `pds.toml` for reading just `[dependencies]`.
#[derive(Debug, Default, Deserialize)]
struct DepsManifest {
    #[serde(default)]
    dependencies: BTreeMap<String, DepSpec>,
}

/// Parses the `[dependencies]` table from a `pds.toml` string.
///
/// # Errors
///
/// Returns an error if the text is not valid TOML.
pub fn parse_dependencies(manifest_toml: &str) -> Result<BTreeMap<String, DepSpec>> {
    let manifest: DepsManifest = toml::from_str(manifest_toml).context("parsing `pds.toml`")?;
    Ok(manifest.dependencies)
}

// ---------------------------------------------------------------------------
// Resolved package identity + lockfile
// ---------------------------------------------------------------------------

/// A resolved package's identity (§8.4): normalised source, resolved commit,
/// and in-repo path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageId {
    pub source: String,
    pub rev: String,
    pub path: String,
}

impl PackageId {
    /// The `pds_modules/` sub-directory this package materialises into —
    /// readable and unique per identity.
    #[must_use]
    pub fn slug(&self) -> String {
        let repo = repo_slug(&self.source);
        let short = &self.rev[..self.rev.len().min(12)];
        if self.path.is_empty() {
            format!("{repo}-{short}")
        } else {
            format!("{repo}-{}-{short}", self.path.replace('/', "_"))
        }
    }
}

/// The parsed `pds.lock` (§8.4).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Lock {
    pub version: u32,
    /// The consumer workspace's direct-dependency edges (name → package). These
    /// are the only dependencies addressable from the consumer's models (§8.3);
    /// the loader maps each name to its package through here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root: Vec<LockEdge>,
    #[serde(default, rename = "package")]
    pub packages: Vec<LockPackage>,
}

/// One resolved package in `pds.lock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockPackage {
    /// The dependency name that introduced this package (informational).
    pub name: String,
    pub source: String,
    pub rev: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<LockEdge>,
}

/// A dependency edge: the name a package declares, and the package it resolves
/// to (`(source, rev, path)`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEdge {
    pub name: String,
    pub source: String,
    pub rev: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
}

impl LockPackage {
    #[must_use]
    pub fn id(&self) -> PackageId {
        PackageId {
            source: self.source.clone(),
            rev: self.rev.clone(),
            path: self.path.clone(),
        }
    }
}

impl LockEdge {
    /// The package identity this edge resolves to.
    #[must_use]
    pub fn id(&self) -> PackageId {
        PackageId {
            source: self.source.clone(),
            rev: self.rev.clone(),
            path: self.path.clone(),
        }
    }
}

/// Parses a `pds.lock` string.
///
/// # Errors
///
/// Returns an error if the text is not valid TOML.
pub fn parse_lock(lock_toml: &str) -> Result<Lock> {
    toml::from_str(lock_toml).context("parsing `pds.lock`")
}

// ---------------------------------------------------------------------------
// Pure resolver
// ---------------------------------------------------------------------------

/// One dependency module's in-memory form: its FQN *within* the dependency
/// workspace (path relative to that workspace root, `::`-joined, no extension)
/// and its source. The dependency name is prefixed by the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepFile {
    pub fqn: String,
    pub source: String,
}

impl DepFile {
    pub fn new(fqn: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            fqn: fqn.into(),
            source: source.into(),
        }
    }
}

/// Maps direct dependencies to their dependency-name-prefixed
/// [`WorkspaceModule`]s (`LANG.md` §8.3) — the single source of the
/// slug→name→`dep::module` mapping, shared by the native loader and the WASM
/// bridge.
///
/// `lock_toml` is the consumer's `pds.lock` (empty/blank when absent — then no
/// git dependencies contribute). `vendored` is each git package file paired with
/// its `pds_modules/` slug; the resolver selects the files whose slug matches a
/// lock edge and prefixes them with that edge's name. `local` is each
/// local-source dependency file paired with the dependency name (ADR-026). Both
/// are flat `(key, file)` lists, so neither caller pre-groups.
///
/// # Errors
///
/// Returns an error if `lock_toml` is present but not valid TOML.
pub fn resolve_dependency_modules(
    lock_toml: &str,
    vendored: &[(String, DepFile)],
    local: &[(String, DepFile)],
) -> Result<Vec<WorkspaceModule>> {
    let mut modules = Vec::new();

    if !lock_toml.trim().is_empty() {
        let lock = parse_lock(lock_toml)?;
        for edge in &lock.root {
            let slug = edge.id().slug();
            for (_, file) in vendored.iter().filter(|(s, _)| *s == slug) {
                modules.push(prefixed(&edge.name, file));
            }
        }
    }

    for (name, file) in local {
        modules.push(prefixed(name, file));
    }

    Ok(modules)
}

/// Prefixes a dependency file's FQN with `name::` (`LANG.md` §8.3).
fn prefixed(name: &str, file: &DepFile) -> WorkspaceModule {
    WorkspaceModule::new(format!("{name}::{}", file.fqn), file.source.clone())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Normalises a relative dependency path and rejects traversal: trims
/// surrounding slashes, then ensures every component is a plain name (no `..`,
/// no absolute root). Returns the cleaned path (`""` for the repo root).
///
/// A git `path` addresses a directory *inside* the cloned repository, so `..`
/// and absolute components must be rejected — they would escape the checkout and
/// `sparse-checkout` would reject them with an opaque error.
///
/// # Errors
///
/// Returns an error if the path is absolute or contains a `..` component.
pub fn sanitize_rel_path(path: &str) -> Result<String> {
    if Path::new(path).is_absolute() {
        bail!("`{path}` must be a relative path inside the repo, not absolute");
    }
    let trimmed = path.trim_matches('/');
    let ok = Path::new(trimmed)
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !ok {
        bail!("`{path}` must be a relative path inside the repo, without `..` or a leading `/`");
    }
    Ok(trimmed.to_owned())
}

/// Normalises a git URL for identity: trims a trailing `/` and one `.git`.
#[must_use]
pub fn normalize_source(url: &str) -> String {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .to_owned()
}

/// The repository's last path segment (no `.git`), e.g.
/// `https://x/acme/banking.git` → `banking`.
#[must_use]
pub fn repo_slug(url: &str) -> String {
    normalize_source(url)
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("dep")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_rejects_multiple_flags() {
        assert!(Rev::from_flags(Some("v1".into()), Some("abc".into()), None).is_err());
        assert_eq!(Rev::from_flags(None, None, None).unwrap(), Rev::Default);
        assert_eq!(
            Rev::from_flags(Some("v1".into()), None, None).unwrap(),
            Rev::Tag("v1".into())
        );
    }

    #[test]
    fn dep_spec_parses_from_toml() {
        let src = r#"
            [dependencies]
            banking = { git = "https://x/acme/banking.git", tag = "v2.1.0", path = "model" }
            utils = { git = "https://x/acme/utils" }
        "#;
        let deps = parse_dependencies(src).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(
            deps["banking"].source("banking").unwrap(),
            Source::Git {
                url: "https://x/acme/banking.git".into(),
                selector: Rev::Tag("v2.1.0".into()),
                sub: "model".into(),
            }
        );
        assert_eq!(
            deps["utils"].source("utils").unwrap(),
            Source::Git {
                url: "https://x/acme/utils".into(),
                selector: Rev::Default,
                sub: String::new(),
            }
        );
    }

    #[test]
    fn local_source_when_path_and_no_git() {
        let deps =
            parse_dependencies("[dependencies]\nshared = { path = \"../shared\" }\n").unwrap();
        assert_eq!(
            deps["shared"].source("shared").unwrap(),
            Source::Local {
                path: "../shared".into(),
            }
        );
    }

    #[test]
    fn no_source_is_rejected() {
        let spec = toml::from_str::<DepSpec>(r#"tag = "v1""#).unwrap();
        let err = spec.source("dep").unwrap_err().to_string();
        assert!(err.contains("declares no source"), "{err}");
    }

    #[test]
    fn local_source_rejects_revision_selector() {
        let spec = toml::from_str::<DepSpec>("path = \"../x\"\ntag = \"v1\"").unwrap();
        let err = spec.source("dep").unwrap_err().to_string();
        assert!(err.contains("takes no"), "{err}");
    }

    #[test]
    fn path_traversal_is_rejected() {
        assert!(sanitize_rel_path("../escape").is_err());
        assert!(sanitize_rel_path("a/../../b").is_err());
        assert!(sanitize_rel_path("/abs").is_err());
        assert!(sanitize_rel_path("/model/").is_err());
        assert_eq!(sanitize_rel_path("model/core").unwrap(), "model/core");
        assert_eq!(sanitize_rel_path("").unwrap(), "");
    }

    #[test]
    fn git_dep_rejects_traversal_path() {
        let spec =
            toml::from_str::<DepSpec>("git = \"https://x/y\"\npath = \"../../etc\"").unwrap();
        assert!(spec.source("dep").is_err());
    }

    #[test]
    fn repo_slug_and_normalize() {
        assert_eq!(repo_slug("https://github.com/acme/banking.git"), "banking");
        assert_eq!(repo_slug("git@github.com:acme/banking.git"), "banking");
        assert_eq!(
            normalize_source("https://github.com/acme/banking.git/"),
            "https://github.com/acme/banking"
        );
    }

    #[test]
    fn package_slug_is_identity_unique() {
        let base = PackageId {
            source: "https://x/acme/banking".into(),
            rev: "0123456789abcdef".into(),
            path: String::new(),
        };
        let sub = PackageId {
            path: "model".into(),
            ..base.clone()
        };
        assert_eq!(base.slug(), "banking-0123456789ab");
        assert_eq!(sub.slug(), "banking-model-0123456789ab");
        assert_ne!(base.slug(), sub.slug());
    }

    #[test]
    fn resolver_prefixes_git_modules_by_slug() {
        let lock = r#"
            version = 1
            [[root]]
            name = "banking"
            source = "https://x/acme/banking"
            rev = "0123456789abcdef"
            path = "model"
        "#;
        // slug for (source, rev=0123456789ab.., path=model) → banking-model-0123456789ab
        let vendored = vec![(
            "banking-model-0123456789ab".to_owned(),
            DepFile::new("core", "//! c\npublic system Ledger;\n"),
        )];
        let modules = resolve_dependency_modules(lock, &vendored, &[]).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].fqn, "banking::core");
        assert!(modules[0].source.contains("Ledger"));
    }

    #[test]
    fn resolver_prefixes_local_modules_by_name() {
        let local = vec![(
            "shared".to_owned(),
            DepFile::new("money", "//! m\npublic data Money { amount: number }\n"),
        )];
        let modules = resolve_dependency_modules("", &[], &local).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].fqn, "shared::money");
    }

    #[test]
    fn resolver_empty_without_lock_or_locals() {
        assert!(resolve_dependency_modules("", &[], &[]).unwrap().is_empty());
    }

    #[test]
    fn resolver_ignores_unmatched_vendored_slug() {
        // A vendored dir not named by a lock edge contributes nothing.
        let lock = "version = 1\n";
        let vendored = vec![("stale-abc123".to_owned(), DepFile::new("x", "//! x\n"))];
        assert!(
            resolve_dependency_modules(lock, &vendored, &[])
                .unwrap()
                .is_empty()
        );
    }
}
