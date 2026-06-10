//! Site presentation config (`LANG.md` §9.3, ADR-017).
//!
//! The CLI fills a [`DocConfig`] from the `[doc]` table of `pds.toml`; this
//! crate only reads it. Documentation is automatic — `[doc]` tunes presentation
//! (title, theme, logo), never *what* is documented.

/// The colour scheme the site ships, written as a `data-theme` attribute on the
/// root `<html>` to select a CSS variable set. `System` (the default) follows
/// the OS `prefers-color-scheme` and is overridable by the in-page toggle;
/// `Light` and `Dark` pin the default without removing the toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Follow the OS scheme; the in-page toggle can override per visitor.
    #[default]
    System,
    /// Default to the dark palette (matches the IDE).
    Dark,
    /// Default to the neutral light palette.
    Light,
}

impl Theme {
    /// The `data-theme` attribute value for this theme.
    #[must_use]
    pub fn attr(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }

    /// The emit-crate theme that renders this site theme's diagrams. The HTML
    /// site always renders adaptively — the `--pds-*` variables in `style.css`
    /// recolour every figure under whichever scheme is active — so pinning the
    /// site light or dark restyles diagrams through CSS, not re-rendering.
    #[allow(clippy::unused_self)] // every site theme renders adaptively today; the parameter keeps the call sites theme-driven
    pub(crate) fn emit(self) -> pseudoscript_emit::Theme {
        pseudoscript_emit::Theme::Adaptive
    }

    /// The emit-crate theme for **standalone** diagram files (the Markdown
    /// site's `diagrams/*.svg`): a pinned scheme renders literal colours so the
    /// file matches the configured look anywhere; `System` renders adaptively
    /// and embeds the dark palette behind a `prefers-color-scheme` query.
    pub(crate) fn emit_standalone(self) -> pseudoscript_emit::Theme {
        match self {
            Theme::Light => pseudoscript_emit::Theme::Light,
            Theme::Dark => pseudoscript_emit::Theme::Dark,
            Theme::System => pseudoscript_emit::Theme::Adaptive,
        }
    }
}

/// Site presentation config, filled by the CLI from `[doc]` in `pds.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocConfig {
    /// Site title (`[doc].name`), shown in the header and on the index.
    pub name: String,
    /// The colour theme (`[doc].theme`).
    pub theme: Theme,
    /// Optional logo. The CLI copies the referenced file into the output
    /// directory; this crate embeds it by its **filename** (`<img src="...">`),
    /// so the value here is the source path the CLI resolves.
    pub logo: Option<String>,
    /// Authored Markdown pages grouped for the sidebar (`[[doc.sidebar]]`),
    /// rendered above the auto-generated module tree. Empty unless configured.
    pub docs: Vec<DocGroup>,
}

/// One `[[doc.sidebar]]` group: a heading and its ordered pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocGroup {
    /// The group heading shown in the sidebar.
    pub title: String,
    /// The group's pages, in declaration order.
    pub pages: Vec<DocPage>,
}

/// One authored Markdown page within a [`DocGroup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocPage {
    /// The page title shown in the sidebar and as the page heading.
    pub title: String,
    /// The source path (relative to `pds.toml`), e.g. `docs/guides/setup.md`.
    /// Drives the page's stable output URL — the host that loaded the content
    /// and this crate derive the same slug from it.
    pub path: String,
    /// The page's raw Markdown, rendered to HTML at build time.
    pub markdown: String,
}

impl Default for DocConfig {
    fn default() -> Self {
        Self {
            name: "Documentation".to_owned(),
            theme: Theme::System,
            logo: None,
            docs: Vec::new(),
        }
    }
}

impl DocConfig {
    /// The site-relative filename a logo is embedded by, derived from the
    /// configured path's final segment. `None` when no logo is configured.
    ///
    /// The CLI copies the source file to this same filename at the site root, so
    /// `<img src="<filename>">` resolves under `file://`.
    #[must_use]
    pub fn logo_filename(&self) -> Option<&str> {
        self.logo
            .as_deref()
            .map(|path| path.rsplit(['/', '\\']).next().unwrap_or(path))
    }
}

#[cfg(test)]
mod tests {
    use super::{DocConfig, Theme};

    #[test]
    fn default_is_system_named_documentation_no_logo() {
        let config = DocConfig::default();
        assert_eq!(config.name, "Documentation");
        assert_eq!(config.theme, Theme::System);
        assert!(config.logo.is_none());
    }

    #[test]
    fn html_diagrams_always_render_adaptively() {
        // The HTML site recolours figures through the --pds-* variables, so a
        // pinned light or dark site still renders adaptive SVG.
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            assert_eq!(theme.emit(), pseudoscript_emit::Theme::Adaptive);
        }
        // Standalone files pin their colours; only System stays adaptive.
        assert_eq!(
            Theme::Light.emit_standalone(),
            pseudoscript_emit::Theme::Light
        );
        assert_eq!(
            Theme::System.emit_standalone(),
            pseudoscript_emit::Theme::Adaptive
        );
    }

    #[test]
    fn logo_filename_is_the_final_path_segment() {
        let config = DocConfig {
            logo: Some("media/brand/pds-logo.svg".to_owned()),
            ..DocConfig::default()
        };
        assert_eq!(config.logo_filename(), Some("pds-logo.svg"));
    }

    #[test]
    fn no_logo_yields_no_filename() {
        assert_eq!(DocConfig::default().logo_filename(), None);
    }
}
