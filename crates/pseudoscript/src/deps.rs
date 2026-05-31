//! `pds add` / `pds install` — git dependency management (`LANG.md` §8.4, §8.5,
//! ADR-024).
//!
//! A `[dependencies]` table in `pds.toml` names other workspaces fetched from
//! git. `pds add` resolves the dependency and its transitive graph, writes the
//! entry and a `pds.lock`, and materialises each package under `pds_modules/`.
//! `pds install` restores `pds_modules/` from an existing `pds.lock`.
//!
//! A package's identity is `(source, rev, path)` (§8.5): the same repo at
//! different revisions or sub-paths are distinct packages and coexist. The fetch
//! uses a sparse, partial checkout so only the dependency workspace's
//! sub-directory lands on disk, not the whole repo.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use pseudoscript_model::WorkspaceModule;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The project-root manifest filename (`LANG.md` §8.1).
const MANIFEST: &str = "pds.toml";
/// The lockfile filename (§8.5).
const LOCKFILE: &str = "pds.lock";
/// The project-local directory dependency packages are materialised into.
const VENDOR_DIR: &str = "pds_modules";
/// The `pds.lock` schema version.
const LOCK_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Manifest `[dependencies]` model
// ---------------------------------------------------------------------------

/// A `[dependencies]` entry as written in `pds.toml` (`LANG.md` §8.4).
///
/// The source is selected by `git` (ADR-024, ADR-026): present → a git source,
/// absent → a local source whose `path` names a sibling workspace. `path` is
/// overloaded — under git it is the in-repo subdirectory, under local it is the
/// manifest-relative directory.
#[derive(Debug, Clone, Deserialize)]
struct DepSpec {
    /// The git source URL, or `None` for a local source.
    git: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    branch: Option<String>,
    /// Under a git source: the dependency workspace's directory within the repo
    /// (default = root). Under a local source: the sibling workspace's path,
    /// relative to the declaring manifest.
    path: Option<String>,
}

/// The resolved source of a `[dependencies]` entry (ADR-026): a fetched git
/// repository, or a local sibling workspace read live from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// A git source: a remote URL, a revision selector, and an in-repo subdir.
    Git {
        url: String,
        selector: Rev,
        sub: String,
    },
    /// A local source: a manifest-relative path to a sibling workspace.
    Local { path: String },
}

/// A resolved revision selector: at most one of tag/rev/branch (§8.4).
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
    fn to_flags(&self) -> (Option<String>, Option<String>, Option<String>) {
        match self {
            Self::Tag(t) => (Some(t.clone()), None, None),
            Self::Commit(r) => (None, Some(r.clone()), None),
            Self::Branch(b) => (None, None, Some(b.clone())),
            Self::Default => (None, None, None),
        }
    }

    /// A stable key distinguishing selectors, for the fetch cache.
    fn key(&self) -> String {
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
    fn source(&self, name: &str) -> Result<Source> {
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

/// Reads the `[dependencies]` table from the manifest at `path`.
fn read_dependencies(path: &Path) -> Result<BTreeMap<String, DepSpec>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading `{}`", path.display()))?;
    let manifest: DepsManifest =
        toml::from_str(&text).with_context(|| format!("parsing `{}`", path.display()))?;
    Ok(manifest.dependencies)
}

// ---------------------------------------------------------------------------
// Resolved package identity + lockfile
// ---------------------------------------------------------------------------

/// A resolved package's identity (§8.5): normalised source, resolved commit,
/// and in-repo path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PackageId {
    source: String,
    rev: String,
    path: String,
}

impl PackageId {
    /// The `pds_modules/` sub-directory this package materialises into —
    /// readable and unique per identity.
    fn slug(&self) -> String {
        let repo = repo_slug(&self.source);
        let short = &self.rev[..self.rev.len().min(12)];
        if self.path.is_empty() {
            format!("{repo}-{short}")
        } else {
            format!("{repo}-{}-{short}", self.path.replace('/', "_"))
        }
    }
}

/// The parsed `pds.lock` (§8.5).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Lock {
    version: u32,
    /// The consumer workspace's direct-dependency edges (name → package). These
    /// are the only dependencies addressable from the consumer's models (§8.4);
    /// the loader maps each name to its package through here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    root: Vec<LockEdge>,
    #[serde(default, rename = "package")]
    packages: Vec<LockPackage>,
}

/// One resolved package in `pds.lock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockPackage {
    /// The dependency name that introduced this package (informational).
    name: String,
    source: String,
    rev: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<LockEdge>,
}

/// A dependency edge: the name a package declares, and the package it resolves
/// to (`(source, rev, path)`).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockEdge {
    name: String,
    source: String,
    rev: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    path: String,
}

impl LockPackage {
    fn id(&self) -> PackageId {
        PackageId {
            source: self.source.clone(),
            rev: self.rev.clone(),
            path: self.path.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// `pds add`
// ---------------------------------------------------------------------------

/// `pds add`: fetch a git dependency, resolve its transitive graph, record it in
/// `pds.toml` + `pds.lock`, and materialise `pds_modules/`.
///
/// # Errors
///
/// Returns an error if no workspace root is found, a fetch fails, the graph has
/// a cycle, or a manifest cannot be read or written.
pub fn add(
    start: &Path,
    url: &str,
    selector: &Rev,
    path: Option<String>,
    name: Option<String>,
) -> Result<()> {
    let root = crate::workspace::find_root(start)?;
    let manifest_path = root.join(MANIFEST);

    let dep_name = name.unwrap_or_else(|| repo_slug(url));
    validate_dep_name(&dep_name)?;
    let spec = spec_from(url, selector, path);

    println!("Adding dependency `{dep_name}` = {url}");

    // Resolve the existing graph plus the new dependency, then persist.
    let mut deps = read_existing_dependencies(&manifest_path)?;
    deps.insert(dep_name.clone(), spec.clone());
    let (root_edges, packages) = resolve_all(&root, &deps)?;

    write_dependency(&manifest_path, &dep_name, &spec)?;
    write_lock(&root.join(LOCKFILE), &root_edges, &packages)?;
    ensure_gitignore(&root)?;

    println!(
        "Resolved {} package(s); wrote {MANIFEST}, {LOCKFILE}, and {VENDOR_DIR}/.",
        packages.len()
    );
    Ok(())
}

/// `pds install`: restore `pds_modules/` from an existing `pds.lock`.
///
/// # Errors
///
/// Returns an error if the lockfile is missing or unreadable, or a fetch fails.
pub fn install(start: &Path) -> Result<()> {
    let root = crate::workspace::find_root(start)?;
    let lock = read_lock(&root.join(LOCKFILE))?;
    let vendor = root.join(VENDOR_DIR);

    let mut restored = 0usize;
    for pkg in &lock.packages {
        let id = pkg.id();
        let dest = vendor.join(id.slug());
        // Present and pinned to the locked commit → trust it. A present but
        // drifted checkout (wrong HEAD, or a manifest with no git metadata) is
        // re-fetched rather than trusted, so `install` is reproducible.
        if workspace_manifest(&dest, &id.path).is_file() {
            match checkout_head(&dest) {
                Ok(head) if head == id.rev => continue,
                Ok(head) => println!(
                    "  re-fetching {} (HEAD {} != locked {})",
                    id.source,
                    short_rev(&head),
                    short_rev(&id.rev)
                ),
                Err(_) => println!("  re-fetching {} (modified dependency)", id.source),
            }
            fs::remove_dir_all(&dest)
                .with_context(|| format!("removing stale `{}`", dest.display()))?;
        } else {
            println!("  fetching {} @ {}", id.source, short_rev(&id.rev));
        }
        fetch_package(
            &id.source,
            &Rev::Commit(id.rev.clone()),
            &id.path,
            &vendor,
            &id,
        )?;
        restored += 1;
    }
    println!(
        "Installed {restored} package(s) into {VENDOR_DIR}/ ({} total).",
        lock.packages.len()
    );
    Ok(())
}

/// `pds update`: re-resolve git dependencies from their manifest selectors,
/// repinning `pds.lock` to the current commit of each moving `branch`/`tag`
/// (and re-fetching as needed). Local dependencies are unaffected — they are not
/// lock-pinned (ADR-026).
///
/// # Errors
///
/// Returns an error if no workspace root is found, a fetch fails, the graph has
/// a cycle, or a manifest cannot be read or written.
pub fn update(start: &Path) -> Result<()> {
    let root = crate::workspace::find_root(start)?;
    let manifest_path = root.join(MANIFEST);
    let deps = read_existing_dependencies(&manifest_path)?;

    // Drop any cached checkouts so a moving ref re-resolves to its current tip
    // rather than reusing the previously fetched commit.
    let vendor = root.join(VENDOR_DIR);
    if vendor.exists() {
        fs::remove_dir_all(&vendor).with_context(|| format!("clearing `{}`", vendor.display()))?;
    }

    let (root_edges, packages) = resolve_all(&root, &deps)?;
    write_lock(&root.join(LOCKFILE), &root_edges, &packages)?;
    ensure_gitignore(&root)?;
    println!(
        "Updated {} git package(s); rewrote {LOCKFILE}.",
        packages.len()
    );
    Ok(())
}

/// `pds remove`: drop a `[dependencies]` entry from `pds.toml`, then re-resolve
/// and rewrite `pds.lock` for the remaining dependencies.
///
/// # Errors
///
/// Returns an error if no workspace root is found, the dependency is not
/// declared, a fetch fails, or a manifest cannot be read or written.
pub fn remove(start: &Path, name: &str) -> Result<()> {
    let root = crate::workspace::find_root(start)?;
    let manifest_path = root.join(MANIFEST);
    let mut deps = read_existing_dependencies(&manifest_path)?;
    if deps.remove(name).is_none() {
        bail!("no dependency named `{name}` in {MANIFEST}");
    }

    remove_dependency(&manifest_path, name)?;
    let (root_edges, packages) = resolve_all(&root, &deps)?;
    write_lock(&root.join(LOCKFILE), &root_edges, &packages)?;
    println!("Removed dependency `{name}`; rewrote {MANIFEST} and {LOCKFILE}.");
    Ok(())
}

/// Loads the modules of every *direct* dependency (§8.4) for cross-workspace
/// resolution, each module's FQN prefixed with the dependency name (so the
/// consumer addresses `dep::module::Node`). Returns an empty list when the
/// workspace declares no dependencies.
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
    let mut modules = Vec::new();
    modules.extend(git_dependency_modules(root)?);
    modules.extend(local_dependency_modules(root)?);
    Ok(modules)
}

/// Loads each direct *git* dependency's modules from `pds_modules/`, prefixed
/// with the dependency name. Empty when the workspace has no `pds.lock`.
fn git_dependency_modules(root: &Path) -> Result<Vec<WorkspaceModule>> {
    let lock_path = root.join(LOCKFILE);
    if !lock_path.is_file() {
        return Ok(Vec::new());
    }
    let lock = read_lock(&lock_path)?;
    let vendor = root.join(VENDOR_DIR);

    let mut modules = Vec::new();
    for edge in &lock.root {
        let id = PackageId {
            source: edge.source.clone(),
            rev: edge.rev.clone(),
            path: edge.path.clone(),
        };
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
        for module in crate::workspace::load_modules(&ws_root)? {
            modules.push(WorkspaceModule::new(
                format!("{}::{}", edge.name, module.fqn),
                module.source,
            ));
        }
    }
    Ok(modules)
}

/// Loads each direct *local* dependency's modules live from disk (ADR-026),
/// prefixed with the dependency name. Local sources are not lock-pinned, so they
/// are read straight from the manifest's `[dependencies]` table.
fn local_dependency_modules(root: &Path) -> Result<Vec<WorkspaceModule>> {
    let manifest = root.join(MANIFEST);
    if !manifest.is_file() {
        return Ok(Vec::new());
    }
    let mut modules = Vec::new();
    for (name, spec) in read_dependencies(&manifest)? {
        if let Source::Local { path } = spec.source(&name)? {
            let dir = resolve_local(root, &name, &path)?;
            for module in crate::workspace::load_modules(&dir)? {
                modules.push(WorkspaceModule::new(
                    format!("{name}::{}", module.fqn),
                    module.source,
                ));
            }
        }
    }
    Ok(modules)
}

// ---------------------------------------------------------------------------
// Resolution graph
// ---------------------------------------------------------------------------

/// Resolves every direct dependency in `deps` and their transitive graph,
/// returning the consumer's direct-dependency edges and the deterministic,
/// lock-ready package list.
fn resolve_all(
    root: &Path,
    deps: &BTreeMap<String, DepSpec>,
) -> Result<(Vec<LockEdge>, Vec<LockPackage>)> {
    let vendor = root.join(VENDOR_DIR);
    fs::create_dir_all(&vendor).with_context(|| format!("creating `{}`", vendor.display()))?;

    let mut ctx = Resolver {
        vendor,
        packages: BTreeMap::new(),
        fetched: HashMap::new(),
    };
    let mut stack = Vec::new();
    let mut root_edges = Vec::new();
    for (name, spec) in deps {
        match spec.source(name)? {
            Source::Git { url, selector, sub } => {
                let id = ctx.resolve(name, &url, &selector, &sub, &mut stack)?;
                root_edges.push(LockEdge {
                    name: name.clone(),
                    source: id.source,
                    rev: id.rev,
                    path: id.path,
                });
            }
            // A local source is read live and records no lock entry (ADR-026):
            // resolve it now only to validate that it names a workspace.
            Source::Local { path } => {
                resolve_local(root, name, &path)?;
            }
        }
    }
    Ok((root_edges, ctx.packages.into_values().collect()))
}

/// Carries the mutable resolution state: the vendor dir, the resolved package
/// map (keyed by identity, so iteration is deterministic and deduped), and a
/// fetch cache keyed by `(source, selector, path)`.
struct Resolver {
    vendor: PathBuf,
    packages: BTreeMap<PackageId, LockPackage>,
    fetched: HashMap<String, String>,
}

impl Resolver {
    /// Fetches and resolves a git `spec` (introduced as `name`), recursing into
    /// its own dependencies, and returns its package identity.
    ///
    /// A git dependency MUST NOT resolve a local-source dependency of its own:
    /// a fetched checkout cannot follow a sibling `path` out of itself
    /// (ADR-026). Such an entry is rejected when recursed into below.
    fn resolve(
        &mut self,
        name: &str,
        url: &str,
        selector: &Rev,
        sub: &str,
        stack: &mut Vec<PackageId>,
    ) -> Result<PackageId> {
        let source = normalize_source(url);

        let rev = self.fetch(&source, selector, sub)?;
        let id = PackageId {
            source,
            rev,
            path: sub.to_owned(),
        };

        if self.packages.contains_key(&id) {
            return Ok(id); // already fully resolved (dedup / coexisting version)
        }
        if stack.contains(&id) {
            bail!("dependency cycle through `{name}` ({})", id.slug());
        }

        // Resolve this package's own dependencies first, then record it.
        let manifest = workspace_manifest(&self.vendor.join(id.slug()), &id.path);
        if !manifest.is_file() {
            bail!(
                "dependency `{name}` has no `{MANIFEST}` at `{}` — not a workspace",
                if id.path.is_empty() {
                    "<repo root>"
                } else {
                    &id.path
                }
            );
        }
        stack.push(id.clone());
        let mut edges = Vec::new();
        for (child_name, child_spec) in read_dependencies(&manifest)? {
            let child = match child_spec.source(&child_name)? {
                Source::Git { url, selector, sub } => {
                    self.resolve(&child_name, &url, &selector, &sub, stack)?
                }
                // A fetched checkout cannot follow a sibling `path` out of
                // itself: a distributed git dependency must use a git source for
                // its own dependencies (ADR-026).
                Source::Local { .. } => bail!(
                    "dependency `{name}` resolves a local `path` dependency `{child_name}`: \
                     a git dependency cannot have local-source dependencies"
                ),
            };
            edges.push(LockEdge {
                name: child_name,
                source: child.source,
                rev: child.rev,
                path: child.path,
            });
        }
        stack.pop();

        self.packages.insert(
            id.clone(),
            LockPackage {
                name: name.to_owned(),
                source: id.source.clone(),
                rev: id.rev.clone(),
                path: id.path.clone(),
                dependencies: edges,
            },
        );
        Ok(id)
    }

    /// Fetches `(source, selector, sub)` into `pds_modules/`, returning the
    /// resolved commit. Cached so a package fetched once is not re-cloned.
    fn fetch(&mut self, source: &str, selector: &Rev, sub: &str) -> Result<String> {
        let cache_key = format!("{source}\0{}\0{sub}", selector.key());
        if let Some(rev) = self.fetched.get(&cache_key) {
            return Ok(rev.clone());
        }
        let rev = fetch_to_temp(source, selector, sub, &self.vendor)?;
        let id = PackageId {
            source: source.to_owned(),
            rev: rev.clone(),
            path: sub.to_owned(),
        };
        promote_temp(&self.vendor, source, selector, &id)?;
        self.fetched.insert(cache_key, rev.clone());
        Ok(rev)
    }
}

// ---------------------------------------------------------------------------
// git fetch (sparse + partial checkout)
// ---------------------------------------------------------------------------

/// The temp checkout directory for a `(source, selector, sub)` before its rev is
/// known, named by a stable hash so re-resolution reuses it.
fn temp_dir(vendor: &Path, source: &str, selector: &Rev, sub: &str) -> PathBuf {
    let hash = short_hash(&format!("{source}\0{}\0{sub}", selector.key()));
    vendor.join(".fetch").join(hash)
}

/// Clones `(source, selector)` with a sparse, partial checkout of only `sub`,
/// then returns the resolved commit SHA. The result lives in a temp dir.
fn fetch_to_temp(source: &str, selector: &Rev, sub: &str, vendor: &Path) -> Result<String> {
    let dest = temp_dir(vendor, source, selector, sub);
    if dest.exists() {
        fs::remove_dir_all(&dest).ok();
    }
    fs::create_dir_all(dest.parent().unwrap_or(&dest))
        .with_context(|| format!("creating `{}`", dest.display()))?;
    let dest_str = dest.to_string_lossy().into_owned();

    // Blobless, sparse clone — minimal objects, working tree limited to the
    // top level until `sparse-checkout set` narrows it to `sub`.
    let mut clone = vec!["clone", "--filter=blob:none", "--sparse"];
    match selector {
        Rev::Tag(r) | Rev::Branch(r) => clone.extend(["--depth", "1", "--branch", r]),
        Rev::Default => clone.extend(["--depth", "1"]),
        Rev::Commit(_) => {} // a bare SHA needs full history reachable
    }
    clone.extend([source, dest_str.as_str()]);
    git(&clone, None).with_context(|| format!("cloning {source}"))?;

    // Scope the working tree: just the workspace sub-directory, or the whole
    // tree when the workspace is the repo root.
    if sub.is_empty() {
        git(&["-C", &dest_str, "sparse-checkout", "disable"], None).context("disabling sparse")?;
    } else {
        git(
            &["-C", &dest_str, "sparse-checkout", "set", "--no-cone", sub],
            None,
        )
        .with_context(|| format!("sparse-checkout of `{sub}`"))?;
    }

    // A pinned commit may not be the cloned HEAD; check it out explicitly.
    if let Rev::Commit(c) = selector {
        git(&["-C", &dest_str, "checkout", c], None)
            .with_context(|| format!("checking out {c}"))?;
    }

    git(&["-C", &dest_str, "rev-parse", "HEAD"], None).context("resolving HEAD")
}

/// Moves the temp checkout to its identity-keyed home, or discards it if that
/// home already exists (the package was fetched under another selector).
fn promote_temp(vendor: &Path, source: &str, selector: &Rev, id: &PackageId) -> Result<()> {
    let sub = &id.path;
    let temp = temp_dir(vendor, source, selector, sub);
    let final_dir = vendor.join(id.slug());
    if final_dir.exists() {
        fs::remove_dir_all(&temp).ok();
        return Ok(());
    }
    fs::rename(&temp, &final_dir)
        .with_context(|| format!("moving `{}` -> `{}`", temp.display(), final_dir.display()))?;
    Ok(())
}

/// `pds install`'s fetch: clone at the exact recorded commit into its slug dir.
fn fetch_package(
    source: &str,
    selector: &Rev,
    sub: &str,
    vendor: &Path,
    id: &PackageId,
) -> Result<()> {
    fetch_to_temp(source, selector, sub, vendor)?;
    promote_temp(vendor, source, selector, id)?;
    Ok(())
}

/// The checked-out HEAD commit of the git repository at `dir`.
fn checkout_head(dir: &Path) -> Result<String> {
    git(&["-C", &dir.to_string_lossy(), "rev-parse", "HEAD"], None)
}

/// A short, display-friendly prefix of a commit SHA.
fn short_rev(rev: &str) -> &str {
    &rev[..rev.len().min(12)]
}

/// Runs `git` with `args` (optionally in `cwd`), returning trimmed stdout.
fn git(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd
        .args(args)
        .output()
        .context("running `git` (is it installed and on PATH?)")?;
    if !output.status.success() {
        bail!(
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

// ---------------------------------------------------------------------------
// pds.toml / pds.lock / .gitignore I/O
// ---------------------------------------------------------------------------

/// Reads `[dependencies]`, returning an empty map if the table is absent.
fn read_existing_dependencies(manifest: &Path) -> Result<BTreeMap<String, DepSpec>> {
    if manifest.is_file() {
        read_dependencies(manifest)
    } else {
        Ok(BTreeMap::new())
    }
}

/// Inserts or replaces one `[dependencies]` entry in `pds.toml`, preserving the
/// rest of the file (comments, `[doc]`, formatting) via `toml_edit`.
fn write_dependency(manifest: &Path, name: &str, spec: &DepSpec) -> Result<()> {
    use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

    let text = fs::read_to_string(manifest)
        .with_context(|| format!("reading `{}`", manifest.display()))?;
    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("parsing `{}`", manifest.display()))?;

    let deps = doc
        .entry("dependencies")
        .or_insert_with(|| Item::Table(Table::new()));
    let table = deps
        .as_table_mut()
        .context("`[dependencies]` in pds.toml is not a table")?;

    let mut entry = InlineTable::new();
    for (key, value) in [
        ("git", &spec.git),
        ("tag", &spec.tag),
        ("rev", &spec.rev),
        ("branch", &spec.branch),
        ("path", &spec.path),
    ] {
        if let Some(value) = value {
            entry.insert(key, Value::from(value.clone()));
        }
    }
    table.insert(name, Item::Value(Value::InlineTable(entry)));

    fs::write(manifest, doc.to_string())
        .with_context(|| format!("writing `{}`", manifest.display()))
}

/// Removes one `[dependencies]` entry from `pds.toml`, preserving the rest of
/// the file via `toml_edit`.
fn remove_dependency(manifest: &Path, name: &str) -> Result<()> {
    use toml_edit::DocumentMut;

    let text = fs::read_to_string(manifest)
        .with_context(|| format!("reading `{}`", manifest.display()))?;
    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("parsing `{}`", manifest.display()))?;

    if let Some(table) = doc.get_mut("dependencies").and_then(|d| d.as_table_mut()) {
        table.remove(name);
    }
    fs::write(manifest, doc.to_string())
        .with_context(|| format!("writing `{}`", manifest.display()))
}

/// Writes `pds.lock` from the consumer's direct edges and the resolved packages,
/// sorted for determinism.
fn write_lock(path: &Path, root_edges: &[LockEdge], packages: &[LockPackage]) -> Result<()> {
    let mut root: Vec<LockEdge> = root_edges.iter().map(LockEdge::clone).collect();
    root.sort_by(|a, b| a.name.cmp(&b.name));
    let mut packages: Vec<LockPackage> = packages.iter().map(LockPackage::clone).collect();
    packages.sort_by_key(LockPackage::id);
    let lock = Lock {
        version: LOCK_VERSION,
        root,
        packages,
    };
    let text = toml::to_string_pretty(&lock).context("serializing pds.lock")?;
    fs::write(path, text).with_context(|| format!("writing `{}`", path.display()))
}

/// Reads and parses `pds.lock`.
fn read_lock(path: &Path) -> Result<Lock> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading `{}` (run `pds add` first?)", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing `{}`", path.display()))
}

/// Ensures `pds_modules/` is gitignored (it is reconstructable from `pds.lock`).
fn ensure_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    let entry = format!("{VENDOR_DIR}/");
    let current = fs::read_to_string(&path).unwrap_or_default();
    if current
        .lines()
        .any(|line| line.trim() == entry || line.trim() == VENDOR_DIR)
    {
        return Ok(());
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&entry);
    next.push('\n');
    fs::write(&path, next).with_context(|| format!("updating `{}`", path.display()))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Builds a git-source `DepSpec` from CLI inputs.
fn spec_from(url: &str, selector: &Rev, path: Option<String>) -> DepSpec {
    let (tag, rev, branch) = selector.to_flags();
    DepSpec {
        git: Some(url.to_owned()),
        tag,
        rev,
        branch,
        path,
    }
}

/// The workspace manifest path for a fetched package: `<dir>/<path>/pds.toml`.
fn workspace_manifest(dir: &Path, sub: &str) -> PathBuf {
    if sub.is_empty() {
        dir.join(MANIFEST)
    } else {
        dir.join(sub).join(MANIFEST)
    }
}

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
fn sanitize_rel_path(path: &str) -> Result<String> {
    if Path::new(path).is_absolute() {
        bail!("`{path}` must be a relative path inside the repo, not absolute");
    }
    let trimmed = path.trim_matches('/');
    let ok = Path::new(trimmed).components().all(|c| {
        matches!(
            c,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    });
    if !ok {
        bail!("`{path}` must be a relative path inside the repo, without `..` or a leading `/`");
    }
    Ok(trimmed.to_owned())
}

/// Resolves a local-source dependency directory relative to the consumer root
/// and confirms it is a workspace (ADR-026). Returns the resolved workspace dir.
///
/// # Errors
///
/// Returns an error if the resolved path is not a directory or has no `pds.toml`.
fn resolve_local(root: &Path, name: &str, path: &str) -> Result<PathBuf> {
    let dir = root.join(path);
    let manifest = dir.join(MANIFEST);
    if !manifest.is_file() {
        bail!(
            "local dependency `{name}` at `{}` is not a workspace (no `{MANIFEST}`)",
            dir.display()
        );
    }
    Ok(dir)
}

/// A dependency name MUST be a valid FQN root segment (it roots cross-workspace
/// names, §8.4): non-empty, no `::` or path separators.
fn validate_dep_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains("::") || name.contains(['/', '\\', ' ']) {
        bail!(
            "invalid dependency name `{name}`: must be a bare identifier (no `::`, `/`, or spaces)"
        );
    }
    Ok(())
}

/// Normalises a git URL for identity: trims a trailing `/` and one `.git`.
fn normalize_source(url: &str) -> String {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .to_owned()
}

/// The repository's last path segment (no `.git`), e.g.
/// `https://x/acme/banking.git` → `banking`.
fn repo_slug(url: &str) -> String {
    normalize_source(url)
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("dep")
        .to_owned()
}

/// A short hex hash, for temp-dir naming.
fn short_hash(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_modules_loads_direct_deps_prefixed() {
        let root =
            std::env::temp_dir().join(format!("pds-depmods-{}-{}", std::process::id(), "auth"));
        let _ = fs::remove_dir_all(&root);
        let id = PackageId {
            source: "https://x/acme/auth".into(),
            rev: "0123456789abcdef".into(),
            path: "login".into(),
        };
        let ws = root.join(VENDOR_DIR).join(id.slug()).join(&id.path);
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join(MANIFEST), "[doc]\nname = \"auth\"\n").unwrap();
        fs::write(ws.join("core.pds"), "//! c\npublic system Login;\n").unwrap();
        let root_edges = vec![LockEdge {
            name: "auth".into(),
            source: id.source.clone(),
            rev: id.rev.clone(),
            path: id.path.clone(),
        }];
        write_lock(&root.join(LOCKFILE), &root_edges, &[]).unwrap();

        let modules = dependency_modules(&root).unwrap();
        fs::remove_dir_all(&root).ok();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].fqn, "auth::core");
        assert!(modules[0].source.contains("public system Login"));
    }

    #[test]
    fn dependency_modules_empty_without_lockfile() {
        let root = std::env::temp_dir().join(format!("pds-nolock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let modules = dependency_modules(&root).unwrap();
        fs::remove_dir_all(&root).ok();
        assert!(modules.is_empty());
    }

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
        let deps = toml::from_str::<DepsManifest>(src).unwrap().dependencies;
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
        let src = r#"
            [dependencies]
            shared = { path = "../shared" }
        "#;
        let deps = toml::from_str::<DepsManifest>(src).unwrap().dependencies;
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
        let spec = toml::from_str::<DepSpec>(
            r#"path = "../x"
tag = "v1""#,
        )
        .unwrap();
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
        let spec = toml::from_str::<DepSpec>(
            r#"git = "https://x/y"
path = "../../etc""#,
        )
        .unwrap();
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
    fn lock_round_trips() {
        let packages = vec![LockPackage {
            name: "banking".into(),
            source: "https://x/acme/banking".into(),
            rev: "abc123".into(),
            path: "model".into(),
            dependencies: vec![LockEdge {
                name: "shared".into(),
                source: "https://x/acme/shared".into(),
                rev: "def456".into(),
                path: String::new(),
            }],
        }];
        let dir = std::env::temp_dir().join(format!("pds-lock-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LOCKFILE);
        let root_edges = vec![LockEdge {
            name: "banking".into(),
            source: "https://x/acme/banking".into(),
            rev: "abc123".into(),
            path: "model".into(),
        }];
        write_lock(&path, &root_edges, &packages).unwrap();
        let lock = read_lock(&path).unwrap();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(lock.version, LOCK_VERSION);
        assert_eq!(lock.root.len(), 1);
        assert_eq!(lock.root[0].name, "banking");
        assert_eq!(lock.packages.len(), 1);
        assert_eq!(lock.packages[0].path, "model");
        assert_eq!(lock.packages[0].dependencies[0].name, "shared");
    }

    #[test]
    fn rejects_bad_dependency_names() {
        assert!(validate_dep_name("auth").is_ok());
        assert!(validate_dep_name("auth::core").is_err());
        assert!(validate_dep_name("a/b").is_err());
        assert!(validate_dep_name("").is_err());
    }
}
