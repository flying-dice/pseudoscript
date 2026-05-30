# ADR-024 — Cross-workspace git dependencies

**Status:** Accepted
**Affects:** LANG.md §8.1, §8.3, §8.4 (new), §8.5 (new); CONFORMANCE/static

## Context

§8.1 makes `pds.toml` the single root every name resolves from; a model can address only its own workspace. Models need to reuse `public` structure defined in other workspaces, distributed via git. A PseudoScript workspace commonly lives in a *subfolder* of a larger application repo, not at the repo root. No dependency, fetch, or cross-workspace resolution concept exists.

## Decision

- **`[dependencies]` declares git dependencies.** A `pds.toml` `[dependencies]` table names other workspaces. Each entry carries a git source, at most one revision selector (`tag`/`rev`/`branch`; default = the remote's default-branch HEAD), and an optional `path` — the dependency workspace's directory within the repository (default = repo root).
- **A dependency name is an FQN root, scoped to the declaring workspace.** `dep::module::Node` addresses the node at module path `module` (§8.1) within dependency `dep`. The same name MAY denote different dependencies in different workspaces — resolution is per-workspace.
- **Cross-workspace targets MUST be `public`.** A private or missing target MUST be rejected, extending the §8.2 cross-module rule.
- **Only direct dependencies are addressable.** A package's own dependencies are fetched and resolved so it is internally well-formed, but are not nameable from a workspace that does not declare them.
- **Versions coexist side-by-side.** A package's identity is `(source, revision, path)`. Entries resolving to one identity are the same package; entries differing in revision or path are distinct packages and MAY coexist. There is no version unification.
- **`pds.lock` pins the resolved graph.** One entry per package — source, resolved commit, path, and dependency edges — making resolution reproducible.
- **`alias` MAY target a cross-workspace node FQN** (§8.3), with the same dangling/private rejection.

## Consequences

- §8.1: dependency names (§8.4) add roots beyond the file-derived ones.
- §8.4, §8.5 (new): the dependency model, per-workspace resolution, side-by-side identity, and the lockfile.
- §8.3: `alias` targets MAY be cross-workspace.
- CONFORMANCE: cross-workspace cases need multi-workspace fixtures; they land with the dependency loader, like the cross-module §8.2 fixtures already deferred there.
- Tooling (non-normative): `pds add` populates `[dependencies]` and `pds.lock`; the fetch uses a sparse, partial checkout to materialize only the dependency workspace's subdirectory; `pds install` restores from the lock; storage is project-local and gitignored.
- Rejected alternatives:
  - A `use`/`import` statement — contradicts §1.1 ("fully-qualified names everywhere"); `alias` already gives local shorthand.
  - A single flat namespace across all packages — defeats side-by-side versions and per-workspace naming.
  - Transitive dependencies nameable — leaks a package's internals and risks FQN-root collisions across the graph.
  - Error on version conflict, or a version solver — `(source, revision, path)` identity makes coexistence the default: no conflict to resolve, no solver to build.
  - Whole-repo checkout — pulls unrelated source; a sparse partial checkout fetches only the workspace subdirectory.
  - A central registry / package index — git URLs are the source of truth; nothing to host.
