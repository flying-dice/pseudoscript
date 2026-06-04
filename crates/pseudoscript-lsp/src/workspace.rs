//! The server's document store, made workspace-aware (`LANG.md` §8.1).
//!
//! This is the filesystem edge of the language server: it finds the project
//! root (`pds.toml`), loads every `.pds` file, and derives each module's FQN
//! from its path relative to the root. Open editor buffers overlay the on-disk
//! text, so analysis always sees the unsaved state. A file opened outside any
//! `pds.toml` is kept as a standalone module whose FQN comes from its `//!`
//! inner doc (falling back to its file stem).
//!
//! Each file's parse is cached and the resolved [`Workspace`] is memoised, both
//! invalidated on any edit — so a burst of hovers/completions between keystrokes
//! re-parses nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pseudoscript_model::{Workspace, WorkspaceModule, ast, static_diagnostics};
use pseudoscript_syntax::{Diagnostic, parse};
use tower_lsp::lsp_types::Url;
use walkdir::WalkDir;

/// The project-root manifest filename (`LANG.md` §8.1).
const MANIFEST: &str = "pds.toml";

/// One known `.pds` file: its FQN, current source, cached parse, and whether an
/// editor buffer is open for it (an open buffer overlays the on-disk text).
#[derive(Debug, Clone)]
struct Entry {
    fqn: String,
    source: String,
    open: bool,
    ast: ast::Module,
    parse_diagnostics: Vec<Diagnostic>,
}

/// The set of `.pds` modules the server knows about, keyed by URI.
#[derive(Debug, Default)]
pub struct Project {
    /// The project root (`pds.toml`'s directory), if one was found.
    root: Option<PathBuf>,
    files: HashMap<Url, Entry>,
    /// FQN → URI, rebuilt on every edit so a resolved target maps to one file.
    by_fqn: HashMap<String, Url>,
    /// Memoised resolved workspace; `None` after any edit, rebuilt on demand.
    resolved: Option<Workspace>,
    /// Direct-dependency modules (`LANG.md` §8.3), each FQN prefixed with the
    /// dependency name. Indexed as externals for cross-workspace resolution.
    external: Vec<WorkspaceModule>,
    /// Whether [`Self::external`] is current. Unlike `resolved`, it survives a
    /// keystroke: dependencies change only with `pds.lock`/`pds.toml`/
    /// `pds_modules`, not on every edit.
    externals_loaded: bool,
}

impl Project {
    /// Discovers the project rooted at or above `root_uri` and loads every
    /// `.pds` file under it. A `root_uri` that resolves to no `pds.toml` leaves
    /// the project empty; standalone files are then added as they open.
    pub fn discover(&mut self, root_uri: &Url) {
        let Ok(dir) = root_uri.to_file_path() else {
            return;
        };
        let Some(root) = find_root(&dir) else {
            return;
        };
        self.load_root(&root);
    }

    /// Walks `root` for `.pds` files, recording each with its path-derived FQN.
    /// Open buffers keep their unsaved text.
    fn load_root(&mut self, root: &Path) {
        self.root = Some(root.to_path_buf());
        self.externals_loaded = false;
        for (uri, source) in disk_sources(root) {
            if self.files.get(&uri).is_some_and(|e| e.open) {
                continue;
            }
            let entry = self.make_entry(&uri, source, false);
            self.files.insert(uri, entry);
        }
        self.reindex();
    }

    /// Records an opened buffer's text, discovering its project root on first
    /// sight so a file opened cold still sees its siblings.
    pub fn open(&mut self, uri: Url, text: String) {
        if self.root.is_none() {
            self.discover(&uri);
        }
        self.invalidate_externals_if_dependency(&uri);
        let entry = self.make_entry(&uri, text, true);
        self.files.insert(uri, entry);
        self.reindex();
    }

    /// Replaces an open buffer's text (full-sync change).
    pub fn change(&mut self, uri: Url, text: String) {
        self.invalidate_externals_if_dependency(&uri);
        let entry = self.make_entry(&uri, text, true);
        self.files.insert(uri, entry);
        self.reindex();
    }

    /// Marks a buffer closed. The editor's unsaved overlay is gone, so on-disk
    /// text is authoritative again: re-read it when present, else drop the entry.
    pub fn close(&mut self, uri: &Url) {
        self.invalidate_externals_if_dependency(uri);
        match uri
            .to_file_path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
        {
            Some(source) => {
                let entry = self.make_entry(uri, source, false);
                self.files.insert(uri.clone(), entry);
            }
            None => {
                self.files.remove(uri);
            }
        }
        self.reindex();
    }

    /// Builds an [`Entry`] for `source`: derives its FQN and caches its parse.
    fn make_entry(&self, uri: &Url, source: String, open: bool) -> Entry {
        let fqn = self.fqn_for(uri);
        let parsed = parse(&source);
        Entry {
            fqn,
            source,
            open,
            ast: parsed.ast,
            parse_diagnostics: parsed.diagnostics,
        }
    }

    /// Rebuilds the FQN index and invalidates the resolved-workspace cache.
    fn reindex(&mut self) {
        self.by_fqn = self
            .files
            .iter()
            .map(|(uri, entry)| (entry.fqn.clone(), uri.clone()))
            .collect();
        self.resolved = None;
    }

    /// The current source for `uri`, if known.
    #[must_use]
    pub fn source(&self, uri: &Url) -> Option<&str> {
        self.files.get(uri).map(|e| e.source.as_str())
    }

    /// The module FQN bound to `uri`, if known.
    #[must_use]
    pub fn fqn(&self, uri: &Url) -> Option<&str> {
        self.files.get(uri).map(|e| e.fqn.as_str())
    }

    /// The URI for module `fqn`, for mapping a resolved target back to a file.
    #[must_use]
    pub fn uri_of(&self, fqn: &str) -> Option<Url> {
        self.by_fqn.get(fqn).cloned()
    }

    /// The source of module `fqn`, for ranging a cross-file target.
    #[must_use]
    pub fn source_of(&self, fqn: &str) -> Option<&str> {
        self.by_fqn
            .get(fqn)
            .and_then(|uri| self.files.get(uri))
            .map(|e| e.source.as_str())
    }

    /// Every module as an owned `(fqn, source)` pair, for whole-workspace scans
    /// (references, rename).
    #[must_use]
    pub fn module_pairs(&self) -> Vec<(String, String)> {
        self.files
            .values()
            .map(|e| (e.fqn.clone(), e.source.clone()))
            .collect()
    }

    /// An owned `fqn → (uri, source)` map, for features that must compute while
    /// holding a workspace borrow (workspace-symbol search).
    #[must_use]
    pub fn fqn_locations(&self) -> HashMap<String, (Url, String)> {
        self.by_fqn
            .iter()
            .filter_map(|(fqn, uri)| {
                self.files
                    .get(uri)
                    .map(|e| (fqn.clone(), (uri.clone(), e.source.clone())))
            })
            .collect()
    }

    /// The memoised resolved [`Workspace`], built from cached parses on first
    /// use after an edit.
    pub fn workspace(&mut self) -> &Workspace {
        if self.resolved.is_none() {
            self.ensure_externals();
            let local: Vec<(String, ast::Module)> = self
                .files
                .values()
                .map(|e| (e.fqn.clone(), e.ast.clone()))
                .collect();
            let external: Vec<(String, ast::Module)> = self
                .external
                .iter()
                .map(|m| (m.fqn.clone(), parse(&m.source).ast))
                .collect();
            self.resolved = Some(Workspace::build_with_externals(local, external));
        }
        self.resolved.as_ref().expect("resolved just built")
    }

    /// Loads the workspace's direct-dependency modules once per `pds.lock`/
    /// `pds.toml`/`pds_modules` change (§8.3), caching the result. A load
    /// failure (e.g. dependencies not installed) degrades to no externals rather
    /// than failing analysis — the server keeps serving local symbols.
    fn ensure_externals(&mut self) {
        if self.externals_loaded {
            return;
        }
        self.external = self.root.as_deref().map(load_externals).unwrap_or_default();
        self.externals_loaded = true;
    }

    /// Marks the externals cache stale when `uri` is a manifest/lockfile or
    /// lives under `pds_modules/`, so the next [`Self::workspace`] reloads them.
    fn invalidate_externals_if_dependency(&mut self, uri: &Url) {
        if uri_touches_dependencies(uri) {
            self.externals_loaded = false;
        }
    }

    /// Diagnostics for every module, each mapped to its file URI and the LSP
    /// shape. Reuses cached parses and the resolved workspace.
    #[must_use]
    pub fn diagnostics(&mut self) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        self.workspace();
        let workspace = self.resolved.as_ref().expect("resolved just built");
        self.files
            .iter()
            .map(|(uri, entry)| {
                let mut diagnostics = entry.parse_diagnostics.clone();
                diagnostics.extend(static_diagnostics(workspace, &entry.fqn));
                (
                    uri.clone(),
                    pseudoscript_lsp_core::analysis::lsp_diagnostics(&entry.source, &diagnostics),
                )
            })
            .collect()
    }

    /// Derives the FQN for `uri` from its file path (§8.1): the path relative to
    /// the project root when inside one, else the file stem for a standalone
    /// file opened outside any `pds.toml`. The filename is the sole source of a
    /// module's identity; a `//!` inner doc is documentation, never the FQN.
    fn fqn_for(&self, uri: &Url) -> String {
        self.root
            .as_deref()
            .zip(uri.to_file_path().ok())
            .and_then(|(root, path)| path_fqn(root, &path))
            .unwrap_or_else(|| uri_stem(uri))
    }
}

/// The file-stem of a URI (`file:///x/a.pds` → `a`), or the whole URI string if
/// it has no path stem — a last-resort unique key for a standalone file.
fn uri_stem(uri: &Url) -> String {
    uri.to_file_path()
        .ok()
        .and_then(|p| {
            p.file_stem()
                .map(|s| pseudoscript_project::normalize_segment(&s.to_string_lossy()))
        })
        .unwrap_or_else(|| uri.to_string())
}

/// Walks up from `dir` for the nearest ancestor containing `pds.toml`.
fn find_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|d| d.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
}

/// Reads every `.pds` file under `root` as `(uri, source)`, skipping hidden
/// directories, `target/`, and the vendored `pds_modules/` (its dependency
/// modules are loaded separately as externals, not as local files).
fn disk_sources(root: &Path) -> Vec<(Url, String)> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(pseudoscript_project::is_visible)
        .filter_map(std::result::Result::ok)
        .filter(|e| pseudoscript_project::is_pds_file(e.path()))
        .filter_map(|entry| {
            let path = entry.path();
            let source = std::fs::read_to_string(path).ok()?;
            let uri = Url::from_file_path(path).ok()?;
            Some((uri, source))
        })
        .collect()
}

/// Loads the workspace's direct-dependency modules (§8.3), degrading to none on
/// any failure (e.g. dependencies not installed) so the server keeps running.
fn load_externals(root: &Path) -> Vec<WorkspaceModule> {
    match pseudoscript_project::dependency_modules(root) {
        Ok(modules) => modules,
        Err(err) => {
            eprintln!("pds: skipping dependency modules: {err:#}");
            Vec::new()
        }
    }
}

/// Whether `uri` is a manifest/lockfile or lives under `pds_modules/` — a change
/// to which invalidates the externals cache.
fn uri_touches_dependencies(uri: &Url) -> bool {
    let Ok(path) = uri.to_file_path() else {
        return false;
    };
    path.file_name()
        .is_some_and(|name| name == "pds.toml" || name == "pds.lock")
        || path.components().any(|c| c.as_os_str() == "pds_modules")
}

/// The FQN for `path` relative to project `root` (`banking/core.pds` →
/// `banking::core`), via the shared [`pseudoscript_project::module_fqn`] so
/// hyphen normalisation (ADR-031) stays single-sourced. `None` if `path`
/// escapes `root` or has no stem.
fn path_fqn(root: &Path, path: &Path) -> Option<String> {
    pseudoscript_project::module_fqn(path.strip_prefix(root).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_fqn_joins_components() {
        let root = Path::new("/proj");
        assert_eq!(
            path_fqn(root, Path::new("/proj/banking/core.pds")).as_deref(),
            Some("banking::core"),
        );
        assert_eq!(
            path_fqn(root, Path::new("/proj/top.pds")).as_deref(),
            Some("top")
        );
    }

    #[test]
    fn path_fqn_normalises_hyphens() {
        // ADR-031: a kebab-case path maps to an identifier root.
        assert_eq!(
            path_fqn(Path::new("/proj"), Path::new("/proj/web-ide/file-tree.pds")).as_deref(),
            Some("web_ide::file_tree"),
        );
    }

    #[test]
    fn rootless_file_fqn_is_its_stem_not_the_inner_doc() {
        // No project root: the module FQN is the file stem (§8.1), never the
        // `//!` header — the filename is the sole identity.
        let mut project = Project::default();
        let uri = Url::parse("file:///tmp/notes.pds").unwrap();
        project.open(
            uri.clone(),
            "//! banking::core\npublic system S;\n".to_owned(),
        );
        assert_eq!(project.fqn(&uri).unwrap(), "notes");
    }

    #[test]
    fn open_overlays_disk_source_and_indexes_fqn() {
        let mut project = Project::default();
        let uri = Url::parse("file:///proj/a.pds").unwrap();
        project.open(uri.clone(), "//! a\npublic system A;\n".to_owned());
        assert_eq!(project.source(&uri), Some("//! a\npublic system A;\n"));
        assert_eq!(project.fqn(&uri).unwrap(), "a");
        // the FQN index maps back to the same file
        assert_eq!(project.uri_of("a").as_ref(), Some(&uri));
    }

    #[test]
    fn editing_rebuilds_the_resolved_cache() {
        let mut project = Project::default();
        let uri = Url::parse("file:///proj/a.pds").unwrap();
        project.open(uri.clone(), "//! a\npublic system A;\n".to_owned());
        assert!(project.workspace().symbol("a::A").is_some());
        // after an edit the cache is invalidated and rebuilt with the new symbol
        project.change(uri, "//! a\npublic system B;\n".to_owned());
        assert!(project.workspace().symbol("a::A").is_none());
        assert!(project.workspace().symbol("a::B").is_some());
    }

    #[test]
    fn pds_modules_dir_is_not_indexed_as_local() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pds.toml"), "[doc]\nname = \"app\"\n").unwrap();
        std::fs::write(dir.path().join("app.pds"), "//! app\npublic system A;\n").unwrap();
        let vend = dir.path().join("pds_modules/banking-abc123def456/core.pds");
        std::fs::create_dir_all(vend.parent().unwrap()).unwrap();
        std::fs::write(&vend, "//! x\npublic system V;\n").unwrap();

        let mut project = Project::default();
        project.discover(&Url::from_directory_path(dir.path()).unwrap());

        let fqns: Vec<_> = project.module_pairs().into_iter().map(|(f, _)| f).collect();
        assert!(fqns.contains(&"app".to_owned()), "{fqns:?}");
        assert!(
            !fqns.iter().any(|f| f.contains("pds_modules")),
            "vendored file leaked as a local module: {fqns:?}"
        );
    }

    #[test]
    fn locked_dependency_modules_are_offered_as_depname() {
        use pseudoscript_model::deps::PackageId;

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pds.toml"), "[doc]\nname = \"app\"\n").unwrap();
        std::fs::write(dir.path().join("app.pds"), "//! app\npublic system A;\n").unwrap();

        // Materialise a vendored package and lock it under the dependency `banking`.
        let id = PackageId {
            source: "https://x/acme/banking".into(),
            rev: "0123456789abcdef".into(),
            path: String::new(),
        };
        let pkg = dir.path().join("pds_modules").join(id.slug());
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("pds.toml"), "[doc]\nname = \"banking\"\n").unwrap();
        std::fs::write(pkg.join("core.pds"), "//! c\npublic system Ledger;\n").unwrap();
        let lock = format!(
            "version = 1\n\n[[root]]\nname = \"banking\"\nsource = \"{}\"\nrev = \"{}\"\npath = \"\"\n",
            id.source, id.rev
        );
        std::fs::write(dir.path().join("pds.lock"), lock).unwrap();

        let mut project = Project::default();
        project.discover(&Url::from_directory_path(dir.path()).unwrap());

        let ws = project.workspace();
        assert!(
            ws.symbol("banking::core::Ledger").is_some(),
            "dependency symbol not offered under its `depname::module` FQN"
        );
    }

    #[test]
    fn externals_survive_a_keystroke_but_reload_on_a_dependency_change() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pds.toml"), "[doc]\nname = \"app\"\n").unwrap();
        std::fs::write(dir.path().join("app.pds"), "//! app\npublic system A;\n").unwrap();
        let app_uri = Url::from_file_path(dir.path().join("app.pds")).unwrap();
        let lock_uri = Url::from_file_path(dir.path().join("pds.lock")).unwrap();

        let mut project = Project::default();
        project.discover(&Url::from_directory_path(dir.path()).unwrap());
        project.workspace(); // first build loads externals
        assert!(project.externals_loaded);

        // A normal `.pds` edit must NOT invalidate the externals cache (no fs
        // re-walk on every keystroke).
        project.change(app_uri, "//! app\npublic system B;\n".to_owned());
        assert!(
            project.externals_loaded,
            "a non-dependency edit wrongly invalidated externals"
        );

        // Editing the lockfile must invalidate them.
        project.change(lock_uri, "version = 1\n".to_owned());
        assert!(
            !project.externals_loaded,
            "a `pds.lock` change must reload externals"
        );
    }

    #[test]
    fn uri_touches_dependencies_matches_manifests_and_vendored_paths() {
        let touches = |p: &str| uri_touches_dependencies(&Url::from_file_path(p).unwrap());
        assert!(touches("/proj/pds.toml"));
        assert!(touches("/proj/pds.lock"));
        assert!(touches("/proj/pds_modules/banking-abc/core.pds"));
        assert!(!touches("/proj/app.pds"));
        assert!(!touches("/proj/src/core.pds"));
    }
}
