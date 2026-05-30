//! `pds upgrade [VERSION]` — replace the running binary with a GitHub release.
//!
//! Mirrors the `scripts/install.{sh,ps1}` contract: the same asset names
//! (`pds-<target>.{tar.gz,zip}`), the same `SHA256SUMS` integrity check, and the
//! same set of prebuilt targets. With no argument it installs the latest
//! release; given `0.1.0` or `v0.1.0` it installs that exact tag.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// The release repository, matching the install scripts.
const REPO: &str = "flying-dice/pseudoscript";

/// `pds upgrade`: resolve the target tag, then download, verify, and self-replace.
pub fn run(version: Option<String>) -> Result<()> {
    let explicit = version.is_some();
    let target = current_target()?;
    let tag = match version {
        Some(v) => normalize_tag(&v),
        None => fetch_latest_tag()?,
    };

    let current = env!("CARGO_PKG_VERSION");
    if !explicit && tag.trim_start_matches('v') == current {
        println!("pds is already up to date (v{current}).");
        return Ok(());
    }

    println!("Upgrading pds v{current} -> {tag} ({target})...");

    let tmp = make_tmp_dir()?;
    let result = install(&tag, target, &tmp);
    let _ = fs::remove_dir_all(&tmp);
    result?;

    println!("pds upgraded to {tag}.");
    Ok(())
}

/// Downloads the release archive into `tmp`, verifies it against `SHA256SUMS`,
/// extracts the binary, and replaces the running executable.
fn install(tag: &str, target: &str, tmp: &Path) -> Result<()> {
    let asset = format!("pds-{target}.{}", archive_ext());
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");
    let archive = tmp.join(&asset);

    println!("  downloading {asset} ...");
    download(&format!("{base}/{asset}"), &archive)
        .with_context(|| format!("downloading {asset} — does release {tag} exist?"))?;

    println!("  verifying checksum ...");
    let sums = http_get(&format!("{base}/SHA256SUMS"))?
        .into_string()
        .context("reading SHA256SUMS")?;
    verify(&archive, &asset, &sums)?;

    let bin = extract(&archive, bin_in_archive(), tmp)?;
    #[cfg(not(windows))]
    set_executable(&bin)?;

    self_replace::self_replace(&bin).context("replacing the running pds binary")?;
    Ok(())
}

/// The target triple of the running binary, restricted to targets with a
/// prebuilt release asset (see `scripts/install.sh`).
fn current_target() -> Result<&'static str> {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        (os, arch) => bail!(
            "no prebuilt binary for {arch} {os}; \
             build from source: cargo install --git https://github.com/{REPO}"
        ),
    };
    Ok(target)
}

/// The release archive extension for the running platform.
const fn archive_ext() -> &'static str {
    if cfg!(windows) { "zip" } else { "tar.gz" }
}

/// The binary's name inside the release archive.
const fn bin_in_archive() -> &'static str {
    if cfg!(windows) { "pds.exe" } else { "pds" }
}

/// Normalizes a user-supplied version to a release tag: `0.1.0` and `v0.1.0`
/// both yield `v0.1.0`.
fn normalize_tag(version: &str) -> String {
    format!("v{}", version.trim().trim_start_matches('v'))
}

/// Resolves the `latest` release tag via the GitHub API.
fn fetch_latest_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = http_get(&url)?
        .into_string()
        .context("reading release API response")?;
    let json: serde_json::Value =
        serde_json::from_str(&body).context("parsing release API response")?;
    json["tag_name"]
        .as_str()
        .map(str::to_owned)
        .context("release API response had no tag_name — are there any releases yet?")
}

/// Issues a GET with a User-Agent (required by the GitHub API), following
/// redirects to the asset's storage backend.
fn http_get(url: &str) -> Result<ureq::Response> {
    ureq::get(url)
        .set("User-Agent", concat!("pds/", env!("CARGO_PKG_VERSION")))
        .call()
        .with_context(|| format!("GET {url}"))
}

/// Streams `url` to `dest`.
fn download(url: &str, dest: &Path) -> Result<()> {
    let mut reader = http_get(url)?.into_reader();
    let mut file =
        fs::File::create(dest).with_context(|| format!("creating `{}`", dest.display()))?;
    std::io::copy(&mut reader, &mut file).context("writing download")?;
    Ok(())
}

/// Verifies `archive` against the `name` entry in `SHA256SUMS`, failing closed
/// if the entry is missing or the digest differs.
fn verify(archive: &Path, name: &str, sums: &str) -> Result<()> {
    let expected =
        expected_hash(sums, name).with_context(|| format!("no checksum listed for {name}"))?;

    let bytes = fs::read(archive).context("reading archive for checksum")?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hasher.finalize();

    let mut actual_hex = String::with_capacity(64);
    for byte in actual {
        write!(actual_hex, "{byte:02x}").expect("writing to a String cannot fail");
    }

    if actual_hex != expected {
        bail!("checksum mismatch for {name}\n  expected: {expected}\n  actual:   {actual_hex}");
    }
    Ok(())
}

/// The expected hex digest for `name` in a `sha256sum`-format listing
/// (`<hash>  <filename>` per line), or `None` if absent.
fn expected_hash<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let hash = fields.next()?;
        let file = fields.next()?;
        (file == name).then_some(hash)
    })
}

/// Extracts the entry named `bin` from a gzipped tarball into `dest_dir`.
#[cfg(not(windows))]
fn extract(archive: &Path, bin: &str, dest_dir: &Path) -> Result<PathBuf> {
    let file = fs::File::open(archive).context("opening archive")?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));

    for entry in tar.entries().context("reading archive")? {
        let mut entry = entry.context("reading archive entry")?;
        let is_bin = entry
            .path()
            .ok()
            .as_deref()
            .and_then(Path::file_name)
            .is_some_and(|name| name == bin);
        if is_bin {
            let out = dest_dir.join(bin);
            entry.unpack(&out).context("extracting binary")?;
            return Ok(out);
        }
    }
    bail!("`{bin}` not found in archive")
}

/// Extracts the entry named `bin` from a zip archive into `dest_dir`.
#[cfg(windows)]
fn extract(archive: &Path, bin: &str, dest_dir: &Path) -> Result<PathBuf> {
    let file = fs::File::open(archive).context("opening archive")?;
    let mut zip = zip::ZipArchive::new(file).context("reading archive")?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).context("reading archive entry")?;
        if entry.name().rsplit('/').next() == Some(bin) {
            let out = dest_dir.join(bin);
            let mut out_file = fs::File::create(&out).context("creating extracted binary")?;
            std::io::copy(&mut entry, &mut out_file).context("extracting binary")?;
            return Ok(out);
        }
    }
    bail!("`{bin}` not found in archive")
}

/// Marks `path` executable (`0o755`).
#[cfg(not(windows))]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut perms = fs::metadata(path)
        .context("stat extracted binary")?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).context("chmod extracted binary")?;
    Ok(())
}

/// Creates a fresh per-process temp directory for the download.
fn make_tmp_dir() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("pds-upgrade-{}", std::process::id()));
    fs::create_dir_all(&dir).with_context(|| format!("creating `{}`", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::{expected_hash, normalize_tag};

    #[test]
    fn normalizes_versions_to_v_prefixed_tags() {
        assert_eq!(normalize_tag("0.1.0"), "v0.1.0");
        assert_eq!(normalize_tag("v0.1.0"), "v0.1.0");
        assert_eq!(normalize_tag("  1.2.3  "), "v1.2.3");
    }

    #[test]
    fn finds_the_matching_checksum_line() {
        let sums = "\
aaaa  pds-x86_64-apple-darwin.tar.gz
bbbb  pds-x86_64-unknown-linux-gnu.tar.gz
";
        assert_eq!(
            expected_hash(sums, "pds-x86_64-unknown-linux-gnu.tar.gz"),
            Some("bbbb")
        );
    }

    #[test]
    fn missing_checksum_entry_is_none() {
        let sums = "aaaa  pds-x86_64-apple-darwin.tar.gz\n";
        assert_eq!(expected_hash(sums, "pds-aarch64-apple-darwin.tar.gz"), None);
    }
}
