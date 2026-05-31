//! Verifies the prebuilt Svelte bundles are present before the crate compiles.
//!
//! The bundles (`src/assets/{ssr.js,client.js,style.css}`) are produced by the
//! `web/` package and committed to the repository, so a normal `cargo build`
//! needs no JS toolchain. This script never runs `npm`; it only fails loudly
//! with a regeneration hint when an asset is missing, and re-runs the build
//! when one changes.

use std::path::Path;

fn main() {
    let assets = Path::new("src/assets");
    for file in ["ssr.js", "client.js", "style.css"] {
        let path = assets.join(file);
        println!("cargo::rerun-if-changed={}", path.display());
        assert!(
            path.exists(),
            "missing prebuilt bundle `{}`. Regenerate it with: \
             `cd crates/pseudoscript-doc/web && npm ci && npm run build` \
             (writes ssr.js, client.js, and style.css into ../src/assets).",
            path.display()
        );
    }
}
