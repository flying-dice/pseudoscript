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

use pseudoscript_model::{Workspace, ast, static_diagnostics};
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
        let entry = self.make_entry(&uri, text, true);
        self.files.insert(uri, entry);
        self.reindex();
    }

    /// Replaces an open buffer's text (full-sync change).
    pub fn change(&mut self, uri: Url, text: String) {
        let entry = self.make_entry(&uri, text, true);
        self.files.insert(uri, entry);
        self.reindex();
    }

    /// Marks a buffer closed. The editor's unsaved overlay is gone, so on-disk
    /// text is authoritative again: re-read it when present, else drop the entry.
    pub fn close(&mut self, uri: &Url) {
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
        let fqn = self.fqn_for(uri, &source);
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
            let parsed = self.files.values().map(|e| (e.fqn.clone(), e.ast.clone()));
            self.resolved = Some(Workspace::build(parsed));
        }
        self.resolved.as_ref().expect("resolved just built")
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
                    crate::analysis::lsp_diagnostics(&entry.source, &diagnostics),
                )
            })
            .collect()
    }

    /// Derives the FQN for `uri`: from its path relative to the project root
    /// when inside one (§8.1), else from the `//!` inner doc, falling back to
    /// the file stem so distinct doc-less files never collide on one FQN.
    fn fqn_for(&self, uri: &Url, text: &str) -> String {
        if let Some(fqn) = self
            .root
            .as_deref()
            .zip(uri.to_file_path().ok())
            .and_then(|(root, path)| path_fqn(root, &path))
        {
            return fqn;
        }
        let inner = inner_doc_fqn(text);
        if !inner.is_empty() {
            return inner;
        }
        uri_stem(uri)
    }
}

/// The file-stem of a URI (`file:///x/a.pds` → `a`), or the whole URI string if
/// it has no path stem — a last-resort unique key for a standalone file.
fn uri_stem(uri: &Url) -> String {
    uri.to_file_path()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| uri.to_string())
}

/// Walks up from `dir` for the nearest ancestor containing `pds.toml`.
fn find_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|d| d.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
}

/// Reads every `.pds` file under `root` as `(uri, source)`, skipping hidden
/// directories and `target/`.
fn disk_sources(root: &Path) -> Vec<(Url, String)> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(is_visible)
        .filter_map(std::result::Result::ok)
        .filter(|e| is_pds_file(e.path()))
        .filter_map(|entry| {
            let path = entry.path();
            let source = std::fs::read_to_string(path).ok()?;
            let uri = Url::from_file_path(path).ok()?;
            Some((uri, source))
        })
        .collect()
}

/// Whether a walked entry should be kept: the root itself, and any non-hidden,
/// non-`target` entry.
fn is_visible(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !(name.starts_with('.') || (entry.file_type().is_dir() && name == "target"))
}

/// Whether `path` is a regular `.pds` file.
fn is_pds_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "pds")
}

/// The FQN for `path` relative to project `root`: each path component becomes a
/// `::`-joined segment, the `.pds` extension stripped (`banking/core.pds` →
/// `banking::core`). `None` if `path` escapes `root` or has no stem.
fn path_fqn(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut segments: Vec<String> = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    segments.push(relative.file_stem()?.to_string_lossy().into_owned());
    Some(segments.join("::"))
}

/// The FQN from a module's first `//!` inner doc — its first whitespace token
/// (`//! banking::core — notes` → `banking::core`) — or empty if absent.
fn inner_doc_fqn(text: &str) -> String {
    parse(text)
        .ast
        .inner_docs
        .first()
        .and_then(|doc: &ast::InnerDoc| doc.text.split_whitespace().next())
        .unwrap_or("")
        .to_owned()
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
    fn inner_doc_fqn_reads_first_token() {
        assert_eq!(
            inner_doc_fqn("//! banking::core — notes\n"),
            "banking::core"
        );
        assert_eq!(inner_doc_fqn("public system S;\n"), "");
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
}
