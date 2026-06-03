# ADR-026 — Local path dependencies

**Status:** Accepted
**Affects:** LANG.md §8.3, §8.4

## Context

ADR-024 established git dependencies as the cross-workspace mechanism: a `[dependencies]` entry names another workspace, fetched and pinned by commit. It does not cover the monorepo case — multiple PseudoScript workspaces in one repository, a sibling referencing another. This spec's own repo is already that shape (`pseudoscript/`, `examples/ticketing/`, `examples/patterns/`). Such a reference has no remote to fetch and no commit to pin distinct from the host repository's.

## Decision

`[dependencies]` gains a **local source**: an entry with a `path` and no `git`. The source is selected by the presence of `git` — set → git source, absent → local source. `path` is overloaded across the two: under a git source it is the dependency's subdirectory within the cloned repository (ADR-024); under a local source it is a filesystem path to a sibling workspace, resolved relative to the declaring `pds.toml`. An entry with neither `git` nor `path` declares no source and MUST be rejected.

A local dependency is **not version-pinned and records no `pds.lock` entry**. A git dependency locks on its commit because the remote moves independently; a local `path` names a directory inside the same checkout, which the host repository already versions. A lock entry would be redundant and would fight the monorepo's single-commit atomicity. The resolver reads local dependencies live from disk; a local dependency's identity is its resolved path.

Each declared name is an **FQN root** (§8.1) exactly as for a git source. Cross-workspace resolution and `public` visibility (§8.2) are source-agnostic: `dep::module::Node` resolves identically whether `dep` is fetched or local.

## Consequences

- A sibling workspace resolves with `path = "../shared"` and no `git` — the form Cargo (`path`), npm (`file:`), and Go (`replace`) users already expect.
- A local source MUST NOT be the resolved source of a git dependency: a consumer fetching a git dependency cannot follow that dependency's local `path` entries out of its own checkout. A dependency intended for distribution uses a git source.
- `pds.lock` covers git dependencies only; two checkouts of the same host commit resolve local dependencies identically by construction.
- Conformance: cross-workspace resolution cases are deferred with the dependency loader (ADR-024). A local source adds no resolution case beyond them — behaviour is source-agnostic. The no-source rejection and the no-lock rule are manifest- and lock-layer concerns, outside the `static/` suite.

## Alternatives considered

- **A distinct key (`local = "../x"`) instead of overloading `path`.** Rejected. `path` already means "where the dependency workspace lives" — repo-relative under git, manifest-relative otherwise; one key is fewer concepts and matches the Cargo mental model. A second key invites entries that set both.
- **Pinning local deps in `pds.lock`.** Rejected. There is no commit to pin distinct from the host repository's; a lock entry would duplicate state the VCS already holds.
