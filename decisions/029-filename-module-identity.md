# ADR-029 — The filename is a module's only identity

**Status:** Accepted
**Affects:** LANG.md §8.1

## Context

A module's FQN is its file path relative to `pds.toml` (§8.1) — `banking/core.pds` → `banking::core`. But the path-less single-file checker (`pds check`/`eval`, the editor's per-keystroke linter) has no filename, and it derived a stand-in name from the first token of the `//!` inner doc. That made a documentation comment load-bearing. A header that happened to match a declared name shadowed it: `//! Configuration` over a file holding `container Configuration` made the module *and* the node both `Configuration`, and `feature … for Configuration` then resolved to the module, not the node — "is not a node," from a comment. Two tools could also disagree about a file's name depending on whether they read the path or the header.

## Decision

A module's identity is its file path alone. The `//!` inner doc documents the module (§2.1); it MUST NOT determine the module name.

- The workspace loader, the LSP, and the doc site derive the FQN from the file path (unchanged).
- A standalone file opened outside any `pds.toml` takes its FQN from the file stem, not its `//!` header.
- The path-less single-file check builds an **anonymous** module (no FQN): same-module references resolve, and cross-module resolution is left to the path-keyed workspace.

## Consequences

- LANG.md §8.1 states the filename is the sole identity.
- `Model::build` no longer reads the `//!` header; it builds an anonymous module. The LSP's rootless-file FQN is the file stem.
- The editor's inline linter no longer mislabels a file whose `//!` header collides with a declared name; cross-module diagnostics come from the workspace check, which keys on the real path FQN.
- Rejected alternative: keep the `//!`-derived name as a fallback. It re-introduces the shadowing and the tool-disagreement; the filename is the one identity every surface already shares.
