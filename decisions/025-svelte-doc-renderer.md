# ADR-025 — The Svelte-rendered site is the sole doc renderer

**Status:** Accepted
**Affects:** LANG.md §9.3; ADR-017 (amends)

## Context

ADR-017 made `pds doc` a cargo-doc-style static site with diagrams "embedded as inline SVG," its HTML assembled in Rust. A second renderer was then added: it authors the presentation in Svelte and server-renders it through an embedded `QuickJS` engine, with diagrams as interactive client islands. Running both behind a `--static` flag / `[doc].renderer` key duplicated the page model and split maintenance across two renderers for one deliverable.

## Decision

- **One renderer.** The Svelte SSR renderer is the sole doc renderer. The Rust HTML renderer, the `pds doc --static` flag, and the `[doc].renderer` key are removed.
- **Presentation in Svelte, embedded prebuilt.** The site is authored in a Svelte package (`web/`) and prebuilt into `ssr.js` / `client.js` / `style.css` bundles, committed to the repository and embedded into the binary. `pds doc` needs no JavaScript toolchain at generation time.
- **Server-rendered for first paint.** Each page is server-rendered through an embedded `QuickJS` engine (native build). A wasm host implements the same JSON-in / JSON-out SSR seam against its own JavaScript engine, so `QuickJS` (which compiles C) never enters a wasm build.
- **Diagrams are interactive client islands.** A diagram ships its laid-out `Scene` geometry, not server-embedded SVG; the client renders C4 views as a Svelte Flow graph and sequences as an animated timeline. Each page embeds its props as `window.__DATA__` so the islands hydrate without refetching.
- **The page model is pure data.** The graph projects to per-page props (resolved hrefs, anchors, edge labels, counts precomputed) with no clock or randomness, so a site is byte-identical across runs.
- **The `Scene` IR stays the generation conformance surface.** Layout and the SVG backend are unchanged (ADR-017); the client draws the same `Scene` geometry.

## Consequences

- §9.3: diagrams are "embedded" rather than "embedded as inline SVG"; the deliverable is still a static site — static files, no server, automatic, no per-output configuration.
- ADR-017: amended on the rendering mechanism — the site is Svelte-SSR'd, not Rust-assembled HTML, and diagrams render client-side from `Scene` geometry rather than as inline SVG. Its static-site deliverable, the `[doc]` table (less any renderer key), and "SVG is the only backend; the `Scene` IR is the conformance surface" all stand.
- Output: `style.css` and `client.js` ship once at the site root and every page links them by a depth-relative path; the prior `app.js` is gone.
- Crate structure (non-normative): the model→page infrastructure (the `DocConfig`/`Theme` config, the `Site`/`SiteFile` types, the FQN→URL scheme, graph navigation, HTML escaping) and the renderer live in the single `pseudoscript-doc` crate; the prior Rust-HTML crate is removed.
- Rejected alternatives:
  - Keeping both renderers behind `--static` / `[doc].renderer` — duplicates the page model and doubles maintenance for a presentation the Svelte site supersedes.
  - Server-embedding the SVG (the ADR-017 approach) — precludes pan/zoom and animation; the interactive diagram is the headline (LANG.md §1).
  - Requiring a JS toolchain at `pds doc` time — the prebuilt, embedded bundle keeps generation toolchain-free; `build.rs` only verifies the bundles exist.
  - A Rust template engine for the new presentation — re-implements what the Svelte component tree already expresses and cannot drive the client islands.
