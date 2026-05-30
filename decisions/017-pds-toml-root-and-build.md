# ADR-017 — `pds.toml` is the project root; `pds doc` generates the doc site

**Status:** Accepted
**Affects:** LANG.md §8.1, §9.3; CONFORMANCE/generation

## Context

§8.1 named `workspace.toml` as the file-system anchor FQNs derive from, and §9 (Diagram Generation) had no way to declare what generation produces. The output model needs a single addressable root and a deliverable. A standalone per-diagram build was considered; the chosen deliverable is a documentation *site*, analogous to `cargo doc`.

## Decision

- **`pds.toml` is the sole project root.** It replaces `workspace.toml`. Module FQNs derive from each `.pds` file's path relative to `pds.toml` (§8.1). Every name is addressable from this one location.
- **`pds doc` generates a static documentation site.** Like `cargo doc`, it documents the workspace automatically: an index with the context diagram, a page per module, a section per node (description, tags, visibility, relationships), an embedded container/component diagram per system/container, and a sequence diagram per triggered callable. Diagrams embed as inline SVG. No per-output configuration is required (§9.3).
- **`[doc]` configures the site.** Optional keys: `name` (title), `out` (output dir, default `target/doc`), `logo`, `theme`. Documentation is automatic; `[doc]` only tunes presentation.
- **SVG is the only backend; the `Scene` IR is the conformance surface.** Diagrams are laid out and rendered to SVG. Raw SVG is brittle to golden (float coords, attribute order), so `CONFORMANCE/generation` asserts the laid-out `Scene` IR (nodes, edges, frames, lifelines), not pixels.

## Consequences

- §8.1: `workspace.toml` → `pds.toml`; FQNs are relative to it.
- §9.3: the `pds doc` site model and the `[doc]` config; supersedes the earlier per-image `[build]`/`[[image]]` descriptor proposal.
- `CONFORMANCE/generation` un-defers: the `Scene` IR shape is pinned and cases assert `.scene` goldens; the site embeds the same SVGs.
- Rejected alternatives: keeping both `workspace.toml` and `pds.toml`; a per-diagram `[[image]]` build manifest (a site documents everything by default — explicit descriptors were dropped as redundant); a descriptors-only site (too far from the `cargo doc` model); shipping text backends (Dot/Mermaid/PlantUML) before SVG (the SVG renderer is the headline, LANG.md §1).
