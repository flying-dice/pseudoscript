//! `pds add` / `pds install` — git dependency management (`LANG.md` §8.3, §8.4,
//! ADR-024).
//!
//! A `[dependencies]` table in `pds.toml` names other workspaces fetched from
//! git. `pds add` resolves the dependency and its transitive graph, writes the
//! entry and a `pds.lock`, and materialises each package under `pds_modules/`.
//! `pds install` restores `pds_modules/` from an existing `pds.lock`.
//!
//! This is the **write/fetch** half: the slug/lock/manifest *parsing* and the
//! read-path that loads dependency modules for resolution live in the WASM-safe
//! [`pseudoscript_model::deps`] and the native [`pseudoscript_project`]; the
//! consumer-facing [`dependency_modules`] is re-exported from there.
//!
//! A package's identity is `(source, rev, path)` (§8.4): the same repo at
//! different revisions or sub-paths are distinct packages and coexist. The fetch
//! uses a sparse, partial checkout so only the dependency workspace's
//! sub-directory lands on disk, not the whole repo.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use pseudoscript_model::deps::{
    DepSpec, Lock, LockEdge, LockPackage, PackageId, Source, normalize_source, parse_dependencies,
    parse_lock, repo_slug,
};
use pseudoscript_project::{LOCKFILE, MANIFEST, VENDOR_DIR, resolve_local, workspace_manifest};
use sha2::{Digest, Sha256};

pub use pseudoscript_model::deps::Rev;
pub use pseudoscript_project::dependency_modules;

/// The `pds.lock` schema version.
const LOCK_VERSION: u32 = 1;

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

/// Reads the `[dependencies]` table from the manifest at `path`.
fn read_dependencies(path: &Path) -> Result<BTreeMap<String, DepSpec>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading `{}`", path.display()))?;
    parse_dependencies(&text).with_context(|| format!("parsing `{}`", path.display()))
}

/// Reads `[dependencies]`, returning an empty map if the manifest is absent.
fn read_existing_dependencies(manifest: &Path) -> Result<BTreeMap<String, DepSpec>> {
    if manifest.is_file() {
        read_dependencies(manifest)
    } else {
        Ok(BTreeMap::new())
    }
}

/// Reads and parses `pds.lock`.
fn read_lock(path: &Path) -> Result<Lock> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading `{}` (run `pds add` first?)", path.display()))?;
    parse_lock(&text).with_context(|| format!("parsing `{}`", path.display()))
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
    let mut root: Vec<LockEdge> = root_edges.to_vec();
    root.sort_by(|a, b| a.name.cmp(&b.name));
    let mut packages: Vec<LockPackage> = packages.to_vec();
    packages.sort_by_key(LockPackage::id);
    let lock = Lock {
        version: LOCK_VERSION,
        root,
        packages,
    };
    let text = toml::to_string_pretty(&lock).context("serializing pds.lock")?;
    fs::write(path, text).with_context(|| format!("writing `{}`", path.display()))
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

/// A dependency name MUST be a valid FQN root segment (it roots cross-workspace
/// names, §8.3): non-empty, no `::` or path separators.
fn validate_dep_name(name: &str) -> Result<()> {
    // The name is an FQN root (LANG.md §8.3) used verbatim as `name::module::Node`,
    // so it must be a bare identifier. A `-` is the common trap: a repo slug like
    // `pseudoscript-jetbrains` would default to a name that can never be addressed.
    if name.is_empty() || name.contains("::") || name.contains(['/', '\\', ' ', '-']) {
        bail!(
            "invalid dependency name `{name}`: an FQN root must be a bare identifier \
             (no `-`, `::`, `/`, or spaces). Pass `--name <identifier>`."
        );
    }
    Ok(())
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
    fn dependency_name_must_be_a_bare_identifier() {
        assert!(validate_dep_name("pseudoscript").is_ok());
        assert!(validate_dep_name("std_money").is_ok());
        // A hyphen makes an invalid FQN root — the repo-slug default trap.
        assert!(validate_dep_name("pseudoscript-jetbrains").is_err());
        assert!(validate_dep_name("").is_err());
        assert!(validate_dep_name("acme::core").is_err());
        assert!(validate_dep_name("a/b").is_err());
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LOCKFILE);
        let root_edges = vec![LockEdge {
            name: "banking".into(),
            source: "https://x/acme/banking".into(),
            rev: "abc123".into(),
            path: "model".into(),
        }];
        write_lock(&path, &root_edges, &packages).unwrap();
        let lock = read_lock(&path).unwrap();

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
