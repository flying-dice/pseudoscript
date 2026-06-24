//! The filesystem edge for `pds doc` (`LANG.md` §8.1, §9.3, ADR-017).
//!
//! The library crates ([`pseudoscript_model`], [`pseudoscript_doc`]) stay
//! pure over in-memory modules. This module is where the toolchain touches disk:
//! it finds the project root (`pds.toml`), parses its `[doc]` table into a
//! [`pseudoscript_doc::DocConfig`] plus a resolved output directory, and
//! walks the tree for `*.pds` files, deriving each module's FQN from its path
//! relative to the root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pseudoscript_doc::{DocConfig, DocGroup, DocPage, Theme};

use crate::DocFormat;
use pseudoscript_model::WorkspaceModule;
use serde::Deserialize;

// The filesystem module-walking primitives live in `pseudoscript-project`
// (shared with the stdio LSP); re-exported so `workspace::find_root` /
// `workspace::load_modules` call sites in `main.rs` stay stable.
pub use pseudoscript_project::{MANIFEST, find_root, load_modules, load_modules_with_paths};

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
    /// Each local module's FQN mapped to its workspace-relative path, so a
    /// diagnostic resolves to the real file (`refs #68`). Built alongside
    /// `modules`; absent only for a module the model omits.
    pub module_paths: HashMap<String, PathBuf>,
    /// Direct git-dependency modules (`LANG.md` §8.3), each FQN prefixed with
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

/// Loads the project rooted at `root`: parses `<root>/pds.toml` and walks the
/// tree for modules.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed, if `[doc].theme`
/// is neither `light` nor `dark`, or if a `.pds` file cannot be read.
pub fn load(root: &Path) -> Result<Workspace> {
    let (config, out, doc_format) = load_manifest(root)?;
    let loaded = load_modules_with_paths(root)?;
    let module_paths = loaded
        .iter()
        .map(|m| (m.module.fqn.clone(), m.relative_path.clone()))
        .collect();
    let modules = loaded.into_iter().map(|m| m.module).collect();
    let dependencies = crate::deps::dependency_modules(root)?;
    Ok(Workspace {
        config,
        out_dir: root.join(out),
        modules,
        module_paths,
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
        let theme = self
            .theme
            .as_deref()
            .map_or(Ok(Theme::System), parse_theme)?;
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

/// Parses a `[doc].theme` value: `light`, `dark`, or `system` (the default).
fn parse_theme(value: &str) -> Result<Theme> {
    match value {
        "light" => Ok(Theme::Light),
        "dark" => Ok(Theme::Dark),
        "system" => Ok(Theme::System),
        other => {
            bail!("invalid `[doc].theme` value `{other}`: expected `light`, `dark`, or `system`")
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_defaults_when_doc_table_absent() {
        let (config, out, format) = toml::from_str::<Manifest>("")
            .unwrap()
            .doc
            .resolve(Path::new("/tmp/my-project"))
            .unwrap();
        assert_eq!(config.name, "my-project");
        assert_eq!(config.theme, Theme::System);
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
