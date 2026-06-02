//! The filesystem edge for `pds doc` (`LANG.md` §8.1, §9.3, ADR-017).
//!
//! The library crates ([`pseudoscript_model`], [`pseudoscript_doc`]) stay
//! pure over in-memory modules. This module is where the toolchain touches disk:
//! it finds the project root (`pds.toml`), parses its `[doc]` table into a
//! [`pseudoscript_doc::DocConfig`] plus a resolved output directory, and
//! walks the tree for `*.pds` files, deriving each module's FQN from its path
//! relative to the root.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pseudoscript_doc::{DocConfig, DocGroup, DocPage, Theme};

use crate::DocFormat;
use pseudoscript_model::WorkspaceModule;
use serde::Deserialize;
use walkdir::WalkDir;

/// The project-root manifest filename (`LANG.md` §8.1).
const MANIFEST: &str = "pds.toml";

/// A loaded project: the resolved doc config, the output directory the site is
/// written to, and the workspace's modules.
#[derive(Debug)]
pub struct Workspace {
    /// Site presentation config, filled from `[doc]`.
    pub config: DocConfig,
    /// The directory the generated site is written to (`<root>/<out>`),
    /// resolved from `[doc].out` (default `target/doc`).
    pub out_dir: PathBuf,
    /// The workspace's modules, sorted by FQN for determinism.
    pub modules: Vec<WorkspaceModule>,
    /// Direct git-dependency modules (`LANG.md` §8.4), each FQN prefixed with
    /// the dependency name. Indexed for cross-workspace resolution but not
    /// checked. Empty when the workspace has no `pds.lock`.
    pub dependencies: Vec<WorkspaceModule>,
    /// The workspace's preferred `pds doc` output format (`[doc].format`), when
    /// set. `None` leaves the choice to the CLI (a `--format` flag, else HTML).
    pub doc_format: Option<DocFormat>,
}

/// The raw `pds.toml`, as parsed before mapping into a [`Workspace`].
#[derive(Debug, Default, Deserialize)]
struct Manifest {
    #[serde(default)]
    doc: DocTable,
}

/// The `[doc]` table; every key is optional (`LANG.md` §9.3).
#[derive(Debug, Default, Deserialize)]
struct DocTable {
    name: Option<String>,
    out: Option<String>,
    logo: Option<String>,
    theme: Option<String>,
    format: Option<String>,
    #[serde(default)]
    sidebar: Vec<SidebarTable>,
}

/// One `[[doc.sidebar]]` group: a heading and its ordered page items.
#[derive(Debug, Default, Deserialize)]
struct SidebarTable {
    #[serde(default)]
    title: String,
    #[serde(default)]
    items: Vec<SidebarItem>,
}

/// One `{ title, path }` entry in a `[[doc.sidebar]]` group's `items`.
#[derive(Debug, Default, Deserialize)]
struct SidebarItem {
    #[serde(default)]
    title: String,
    path: String,
}

/// Walks up from `start` (a file or directory) through its ancestors until a
/// directory containing `pds.toml` is found, returning that directory.
///
/// # Errors
///
/// Returns an error if no ancestor of `start` contains a `pds.toml`.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    // A file's own path is not a search directory; begin at its parent.
    let from = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
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

/// Loads the project rooted at `root`: parses `<root>/pds.toml` and walks the
/// tree for modules.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed, if `[doc].theme`
/// is neither `light` nor `dark`, or if a `.pds` file cannot be read.
pub fn load(root: &Path) -> Result<Workspace> {
    let (config, out, doc_format) = load_manifest(root)?;
    let modules = load_modules(root)?;
    let dependencies = crate::deps::dependency_modules(root)?;
    Ok(Workspace {
        config,
        out_dir: root.join(out),
        modules,
        dependencies,
        doc_format,
    })
}

/// Reads and parses `<root>/pds.toml`, mapping its `[doc]` table into a
/// [`DocConfig`] and the (root-relative) output directory.
fn load_manifest(root: &Path) -> Result<(DocConfig, PathBuf, Option<DocFormat>)> {
    let path = root.join(MANIFEST);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading `{}`", path.display()))?;
    let manifest: Manifest =
        toml::from_str(&text).with_context(|| format!("parsing `{}`", path.display()))?;
    manifest.doc.resolve(root)
}

impl DocTable {
    /// Maps the parsed `[doc]` table into a [`DocConfig`] and the root-relative
    /// output directory, applying defaults. Reads each `[[doc.sidebar]]` page's
    /// Markdown from disk, relative to `root`.
    fn resolve(self, root: &Path) -> Result<(DocConfig, PathBuf, Option<DocFormat>)> {
        let name = self.name.unwrap_or_else(|| default_name(root));
        let theme = self.theme.as_deref().map_or(Ok(Theme::Dark), parse_theme)?;
        let format = self.format.as_deref().map(parse_format).transpose()?;
        let out = self.out.unwrap_or_else(|| "target/doc".to_owned());
        let docs = self
            .sidebar
            .into_iter()
            .map(|group| load_doc_group(root, group))
            .collect();
        let config = DocConfig {
            name,
            theme,
            logo: self.logo,
            docs,
        };
        Ok((config, PathBuf::from(out), format))
    }
}

/// Reads a `[[doc.sidebar]]` group's pages from disk into a [`DocGroup`]. A page
/// whose file cannot be read warns and is skipped — like a missing logo, it does
/// not fail `pds doc` (`LANG.md` §9.3). An item's title defaults to its path.
fn load_doc_group(root: &Path, group: SidebarTable) -> DocGroup {
    let pages = group
        .items
        .into_iter()
        .filter_map(|item| {
            let source = root.join(&item.path);
            match std::fs::read_to_string(&source) {
                Ok(markdown) => Some(DocPage {
                    title: if item.title.is_empty() {
                        item.path.clone()
                    } else {
                        item.title
                    },
                    path: item.path,
                    markdown,
                }),
                Err(err) => {
                    eprintln!("warning: doc page `{}`: {err}", source.display());
                    None
                }
            }
        })
        .collect();
    DocGroup {
        title: group.title,
        pages,
    }
}

/// The default site name: the root directory's final component (`LANG.md`
/// §9.3), falling back to `DocConfig::default().name` for a rootless path.
fn default_name(root: &Path) -> String {
    root.file_name().map_or_else(
        || DocConfig::default().name,
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Parses a `[doc].theme` value, rejecting anything but `light`/`dark`.
fn parse_theme(value: &str) -> Result<Theme> {
    match value {
        "light" => Ok(Theme::Light),
        "dark" => Ok(Theme::Dark),
        other => bail!("invalid `[doc].theme` value `{other}`: expected `light` or `dark`"),
    }
}

/// Parses a `[doc].format` value, rejecting anything but `html`/`md`
/// (`markdown` is accepted as an alias for `md`).
fn parse_format(value: &str) -> Result<DocFormat> {
    match value {
        "html" => Ok(DocFormat::Html),
        "md" | "markdown" => Ok(DocFormat::Md),
        other => bail!("invalid `[doc].format` value `{other}`: expected `html` or `md`"),
    }
}

/// Walks `root` recursively for `*.pds` files (skipping `target/` and hidden
/// directories), reading each and deriving its FQN from its path relative to
/// `root`. The result is sorted by FQN for determinism.
///
/// # Errors
///
/// Returns an error if the tree cannot be walked or a `.pds` file read.
pub fn load_modules(root: &Path) -> Result<Vec<WorkspaceModule>> {
    let mut modules = Vec::new();
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
        modules.push(WorkspaceModule::new(fqn, source));
    }
    modules.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    Ok(modules)
}

/// Whether `entry` should be descended into / kept: skips the `target` and
/// `pds_modules` directories (build output and vendored dependencies) and any
/// hidden entry (a name starting with `.`). The walk root itself is always kept.
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

/// Whether `path` is a regular file with a `.pds` extension.
fn is_pds_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "pds")
}

/// Derives a module FQN from a `.pds` path relative to the project root
/// (`LANG.md` §8.1): each path component becomes a `::`-joined segment, with the
/// `.pds` extension stripped from the filename. `banking/core.pds` →
/// `banking::core`; `pseudoscript.pds` → `pseudoscript`.
///
/// Returns `None` when the path has no usable filename stem.
fn module_fqn(relative: &Path) -> Option<String> {
    let mut segments: Vec<String> = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let stem = relative.file_stem()?.to_string_lossy().into_owned();
    segments.push(stem);
    Some(segments.join("::"))
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
    fn manifest_defaults_when_doc_table_absent() {
        let (config, out, format) = toml::from_str::<Manifest>("")
            .unwrap()
            .doc
            .resolve(Path::new("/tmp/my-project"))
            .unwrap();
        assert_eq!(config.name, "my-project");
        assert_eq!(config.theme, Theme::Dark);
        assert!(config.logo.is_none());
        assert_eq!(out, Path::new("target/doc"));
        assert_eq!(format, None);
    }

    #[test]
    fn manifest_reads_explicit_values() {
        let src = r#"
            [doc]
            name = "Banking"
            out = "build/site"
            logo = "media/logo.svg"
            theme = "dark"
        "#;
        let (config, out, _) = toml::from_str::<Manifest>(src)
            .unwrap()
            .doc
            .resolve(Path::new("/tmp/proj"))
            .unwrap();
        assert_eq!(config.name, "Banking");
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.logo.as_deref(), Some("media/logo.svg"));
        assert_eq!(out, Path::new("build/site"));
    }

    #[test]
    fn manifest_rejects_unknown_theme() {
        let err = toml::from_str::<Manifest>("[doc]\ntheme = \"sepia\"\n")
            .unwrap()
            .doc
            .resolve(Path::new("/tmp/proj"))
            .unwrap_err();
        assert!(err.to_string().contains("sepia"), "{err}");
    }

    #[test]
    fn manifest_has_no_docs_when_sidebar_absent() {
        let (config, _, _) = toml::from_str::<Manifest>("[doc]\nname = \"X\"\n")
            .unwrap()
            .doc
            .resolve(Path::new("/tmp/proj"))
            .unwrap();
        assert!(config.docs.is_empty());
    }

    #[test]
    fn manifest_reads_doc_format() {
        let resolve = |toml: &str| {
            toml::from_str::<Manifest>(toml)
                .unwrap()
                .doc
                .resolve(Path::new("/tmp/proj"))
        };
        assert_eq!(resolve("[doc]\n").unwrap().2, None);
        assert_eq!(
            resolve("[doc]\nformat = \"md\"\n").unwrap().2,
            Some(DocFormat::Md)
        );
        assert_eq!(
            resolve("[doc]\nformat = \"markdown\"\n").unwrap().2,
            Some(DocFormat::Md)
        );
        assert_eq!(
            resolve("[doc]\nformat = \"html\"\n").unwrap().2,
            Some(DocFormat::Html)
        );
        assert!(resolve("[doc]\nformat = \"pdf\"\n").is_err());
    }

    #[test]
    fn manifest_reads_sidebar_pages_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/intro.md"), "# Intro\n\nHello.").unwrap();
        let src = r#"
            [doc]
            name = "X"

            [[doc.sidebar]]
            title = "Start"
            items = [
              { title = "Intro", path = "docs/intro.md" },
              { title = "Gone",  path = "docs/missing.md" },
            ]
        "#;
        let (config, _, _) = toml::from_str::<Manifest>(src)
            .unwrap()
            .doc
            .resolve(dir.path())
            .unwrap();
        assert_eq!(config.docs.len(), 1);
        let group = &config.docs[0];
        assert_eq!(group.title, "Start");
        // The missing page is warned-and-skipped; only the readable one survives.
        assert_eq!(group.pages.len(), 1);
        assert_eq!(group.pages[0].title, "Intro");
        assert_eq!(group.pages[0].path, "docs/intro.md");
        assert!(group.pages[0].markdown.contains("# Intro"));
    }
}
