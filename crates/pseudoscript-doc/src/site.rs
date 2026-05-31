//! The generated-site value types.
//!
//! [`try_render_site_with`](crate::try_render_site_with) returns a [`Site`] — a
//! list of in-memory [`SiteFile`]s. This crate performs no filesystem I/O; the
//! CLI writes each file under the configured output directory, joining
//! `SiteFile::path` to it.

/// One generated file: a site-relative path and its full contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteFile {
    /// The path relative to the site root, e.g. `index.html`,
    /// `module/banking.core.html`, `style.css`, `client.js`. Always
    /// `/`-separated.
    pub path: String,
    /// The file's complete contents.
    pub contents: String,
}

impl SiteFile {
    /// Builds a site file from its path and contents.
    pub fn new(path: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: contents.into(),
        }
    }
}

/// A whole generated documentation site: every file it comprises.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Site {
    /// The site's files, in a deterministic order (`index.html`, the shared
    /// assets, then module pages sorted by FQN).
    pub files: Vec<SiteFile>,
}

impl Site {
    /// The file at `path`, if the site has one. Handy for tests and the CLI.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&SiteFile> {
        self.files.iter().find(|f| f.path == path)
    }
}
