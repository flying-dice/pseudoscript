//! Site presentation config (`LANG.md` §9.3, ADR-017).
//!
//! The CLI fills a [`DocConfig`] from the `[doc]` table of `pds.toml`; this
//! crate only reads it. Documentation is automatic — `[doc]` tunes presentation
//! (title, theme, logo), never *what* is documented.

/// The colour theme the site ships, written as a `data-theme` attribute on the
/// root `<html>` and selecting between the light/dark CSS variable sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// The default neutral light palette.
    #[default]
    Light,
    /// The dark palette.
    Dark,
}

impl Theme {
    /// The `data-theme` attribute value for this theme.
    #[must_use]
    pub fn attr(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
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
}

impl Default for DocConfig {
    fn default() -> Self {
        Self {
            name: "Documentation".to_owned(),
            theme: Theme::Light,
            logo: None,
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
    fn default_is_light_named_documentation_no_logo() {
        let config = DocConfig::default();
        assert_eq!(config.name, "Documentation");
        assert_eq!(config.theme, Theme::Light);
        assert!(config.logo.is_none());
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
