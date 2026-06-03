# T2 — Install deps from git (#cli)

## Summary
`pds add <git-url> [--tag|--rev|--branch] [--path SUB] [--name N]` and
`pds install` fetch PseudoScript workspace dependencies from git — including
workspaces that live in a *sub-directory* of a larger monorepo (sparse, blobless
checkout of only that subtree) — pin them to an exact commit in `pds.lock`,
materialise them under `pds_modules/`, and make them resolvable as
dependency-name-prefixed modules in cross-workspace checking.

**Important correction to the backlog premise:** this feature is *not* a stub
with a TODO. `crates/pseudoscript/src/deps.rs` (860 lines) is a complete,
documented implementation — real `git clone --filter=blob:none --sparse` with
`sparse-checkout set --no-cone <sub>`, a versioned TOML lockfile with transitive
package graph + cycle detection, a fetch cache, `(source, rev, path)` package
identity with coexisting versions, `.gitignore` hygiene, consumption via
`dependency_modules` wired into `workspace::load` → `check_workspace_with_externals`,
and 8 unit tests. The task is therefore **hardening + verification + spec/test
coverage**, not green-field construction.

## Current state (file:line) — what already works vs stub

Crate `crates/pseudoscript` (binary `pds`). `deps.rs` is `mod deps` in
`main.rs:17`. Cited from full reads of `deps.rs`, `main.rs`, `workspace.rs`.

CLI surface (works):
- `Add { url, tag, rev, branch, path, name }` with `conflicts_with_all` enforcing
  one-of tag/rev/branch at the clap layer (`main.rs:97-116`); `Install`
  (`main.rs:117-118`). Routed via `cmd_add`/`cmd_install` (`main.rs:136-144`,
  `153-170`), both rooted at `Path::new(".")`.

Manifest + selector model (works):
- `DepSpec { git, tag, rev, branch, path }` deserialized from `[dependencies]`
  (`deps.rs:39-48`, `120-133`).
- `Rev` selector enum with `from_flags` (one-of validation, `deps.rs:66-78`),
  `to_flags` inverse (`82-89`), and a cache `key` (`92-99`).
- `sub_path` normalises the in-repo path; empty = repo root (`deps.rs:111-117`).

`pds add` (works, end to end):
- `add` finds root, derives name (`--name` else `repo_slug`), validates it,
  resolves the *whole* graph incl. the new dep, then writes manifest + lock +
  gitignore (`deps.rs:220-250`).
- `write_dependency` uses `toml_edit` to insert/replace one `[dependencies]`
  entry while preserving comments/`[doc]`/formatting (`deps.rs:582-614`).

Lockfile (works):
- `Lock { version, root: Vec<LockEdge>, package: Vec<LockPackage> }`, version
  const = 1 (`deps.rs:32`, `163-207`). `root` = consumer's direct edges;
  `package` = full transitive set, each carrying its own `dependencies` edges.
- `write_lock` sorts root by name and packages by id for deterministic diffs
  (`deps.rs:618-630`); `read_lock` parses with an actionable missing-file message
  (`633-637`).

git fetch — sparse + partial subtree (works, this is the part the brief assumed
missing):
- `fetch_to_temp`: `git clone --filter=blob:none --sparse`, adding
  `--depth 1 --branch <r>` for tag/branch/default, *no* depth for a bare commit
  (full history reachable) (`deps.rs:478-496`); then `sparse-checkout set
  --no-cone <sub>` for a subdir or `sparse-checkout disable` for repo-root, then
  explicit `checkout <commit>` when pinned, then `rev-parse HEAD` to capture the
  resolved sha (`deps.rs:500-516`). **This is the monorepo-subtree behaviour the
  memory note requires.**
- `temp_dir` hashes `(source, selector, sub)` so re-resolution reuses
  (`deps.rs:471-474`); `promote_temp` atomically `rename`s temp → identity-keyed
  `slug` dir, discarding if the identity already exists (`521-532`).
- `git` helper shells out, bails with the failing args + stderr (`deps.rs:548-565`).

Transitive resolution (works):
- `Resolver` keyed by `PackageId`, with a fetch cache; `resolve` recurses into
  each fetched package's own `[dependencies]`, dedups by identity, and detects
  cycles via a `stack` (`deps.rs:346-463`). Errors if a fetched dep has no
  `pds.toml` ("not a workspace", `409-419`).

Consumption / resolvability (works — already wired):
- `dependency_modules(root)` reads the lock's `root` edges, loads each direct
  dep's `.pds` modules from its `pds_modules/<slug>[/<path>]`, prefixes each FQN
  with the dep name (`dep::module::Node`), and errors "run `pds install`" if a
  dep is absent (`deps.rs:302-337`).
- `workspace::load` calls it into `Workspace.dependencies` (`workspace.rs:107`,
  `32-36`); `build_site` feeds those to `check_workspace_with_externals`
  (`main.rs:389-392`). `load_modules`/`is_visible` skip `pds_modules/` and
  `target/` when walking the *consumer* (`workspace.rs:207-241`).

Hygiene (works): `ensure_gitignore` idempotently appends `pds_modules/`
(`deps.rs:640-657`).

Tests (8 unit tests, `deps.rs:722-860`): dependency_modules prefixing + empty
without lock, selector one-of, DepSpec TOML parse, repo_slug/normalize, slug
identity-uniqueness, lock round-trip, dep-name validation. **All operate on temp
dirs / in-memory; none exercise a real `git` fetch.**

Genuine gaps / weaknesses (the actual work):
- **No integration test of the git path.** `fetch_to_temp`, `promote_temp`,
  sparse subtree, commit pinning, and `install` idempotency are entirely
  untested against a real repo. This is the largest risk.
- **`pds install` re-fetches the full graph but only checks direct presence by
  manifest existence** (`deps.rs:266`), and never verifies the checked-out HEAD
  equals `id.rev`. A tampered/partial `pds_modules` entry is trusted. No
  `rev-parse HEAD == locked` guard.
- **`install` ignores `pds.toml`/lock drift** — it restores exactly what the lock
  says (correct) but there is no command to *re-resolve* a moving `branch`/`tag`
  to a new commit short of editing + re-running `add`. No `pds update`.
- **Bare-commit fetch clones full history** (`deps.rs:493`) — no
  `fetch --depth 1 <sha>` fast path; on large monorepos this is slow. Could try a
  shallow `fetch <sha>` first, fall back to full clone.
- **Temp `.fetch` dir under `pds_modules/`** is not cleaned on failure mid-fetch
  (only on next run via the `dest.exists()` purge, `deps.rs:480-481`); a failed
  `add` can leave `.fetch/<hash>` litter and a partially-written graph (manifest
  written only after resolve succeeds — good — but lock/gitignore ordering means
  a fetch failure aborts before any write, which is correct).
- **No `pds remove`**; re-`add` of an existing name silently overwrites
  (`deps.rs:238` `insert`) — acceptable but undocumented.
- **`--path` validation** is trim-only; a `../escape` path is passed to
  `sparse-checkout` unsanitised (git will reject, but the error is opaque).
- **Spec/conformance:** verify `LANG.md` §8.3/§8.3 and ADR-024 actually describe
  this surface (referenced in `deps.rs:1-2` doc comment); the lockfile schema
  (`root` + `package` arrays, version 1) and the `(source,rev,path)` identity
  should be pinned there. No conformance cases under `CONFORMANCE/` cover deps
  (deps are a CLI/tooling concern, likely out of the four spec layers — confirm).

## Proposed approach
Hardening, not rewrite. In priority order:

1. **Integration test against a local `file://` git fixture** (`tests/deps.rs`,
   new): create a temp git repo with a workspace in a subdir + a sibling subdir,
   `git commit`, then drive `deps::add(start, file_url, &Rev::Default,
   Some("model"), None)`. Assert: `pds.toml` gains the `[dependencies]` entry;
   `pds.lock` has a 40-char `rev`; only `model/` (not the sibling) is materialised;
   `git rev-parse HEAD` in the checkout == the locked rev; `dependency_modules`
   returns the prefixed module. Then a fresh root with the same `pds.lock` →
   `install` reproduces it; a second `install` is a no-op. This closes the biggest
   gap and locks in the monorepo-subtree contract. (Add `tempfile` dev-dep if not
   present; `git` already required at runtime.)

2. **Verify checked-out HEAD in `install`** (`deps.rs:257-288`): after the
   presence check, `git rev-parse HEAD` in `dest/<sub>`'s repo and compare to
   `id.rev`; on mismatch, re-fetch (or bail with a clear "modified dependency"
   message). Strengthens reproducibility (acceptance #4/#5).

3. **Shallow bare-commit fetch** (`deps.rs:489-496`): try `init` + `fetch
   --depth 1 origin <sha>` + `checkout FETCH_HEAD` for `Rev::Commit`, falling back
   to the current full clone if the server refuses uploadpack-by-sha. Optional;
   purely a perf win for monorepos. Defer if it complicates the happy path.

4. **Sanitise `--path`**: reject `..` / absolute components in `sub_path` with a
   clear error before it reaches git (mirror `safe_rel_path` style in
   `main.rs:576-587`).

5. **Spec alignment:** already done. `LANG.md` §8.3 (`[dependencies]` table,
   one-of selector, `path` for monorepo subtrees) and §8.3 (`(source, revision,
   path)` identity, `pds.lock` pinning the graph, sparse partial checkout,
   direct-only addressability, acyclicity) match the code; ADR-024 closes with
   "Spec is fully aligned with the implementation." No spec work required for T2
   unless the `install` HEAD-verify behaviour (step 2) warrants a §8.3 sentence.

6. **Docs:** a `PATTERNS.md` recipe for adding a monorepo-hosted dependency
   (`pds add <repo> --path <subdir>`), and a `pds update` follow-up ticket.

## Affected/new files
- `crates/pseudoscript/tests/deps.rs` (new) — `file://` git fixture integration
  test (the main deliverable).
- `crates/pseudoscript/src/deps.rs` — HEAD-verification in `install`, `--path`
  sanitisation, optional shallow bare-commit fetch.
- `crates/pseudoscript/Cargo.toml` — `tempfile` under `[dev-dependencies]` if not
  already there (no new runtime dep; git is shelled).
- `LANG.md`/ADR-024 — no change needed (already aligned); only touch if step-2
  HEAD-verify adds a normative sentence (spec-style).
- `PATTERNS.md` — monorepo-dependency recipe (spec-style), optional.

## Open questions / decisions needed
- **Cache location:** per-workspace `pds_modules/` (current, Cargo-`vendor`-like)
  vs a shared global cache keyed by `(source,rev,path)`. Current is simple,
  gitignored, reproducible from the lock. Recommend keeping per-workspace; flag
  global de-dup as future.
- **`pds update`:** out of scope for T2, but needed to re-resolve a moving
  `branch`/`tag` to a fresh commit. Confirm it's a separate ticket.
- **Bare-commit shallow fetch:** worth the added branch, or accept full clone for
  v1? (Affects large-monorepo UX, the exact scenario in the memory note.)
- **Tampered-checkout policy in `install`:** re-fetch silently vs bail? Recommend
  re-fetch (self-heal) with an info line.
- **Conformance scope:** git deps are a CLI/tooling concern, outside the four
  spec layers (lexical/syntax/static/generation). The cross-workspace *name*
  rules (§8.3 direct-only addressability) could warrant a `static/` case, but the
  fetch/lock mechanics belong in Rust integration tests, not `CONFORMANCE/`.
  Confirm before adding any case.
- **Outer-repo discovery:** `find_root` stops at the workspace `pds.toml`, not the
  enclosing monorepo (`workspace.rs:79-95`). Fine for the consumer; the *fetch*
  already handles subtree via `--path`. Confirm no case needs the outer git root.

## Dependencies on other tasks (T1 monorepo resolution)
Largely **already integrated** — `dependency_modules` → `Workspace.dependencies`
→ `check_workspace_with_externals` is wired today (`workspace.rs:107`,
`main.rs:389-392`). If T1 ("monorepo resolution") is the cross-workspace
*name-resolution / checking* depth (alias/import across the `dep::` prefix, per
the cross-module-alias memory note and `LANG.md §8.3`), then T2 already produces
the prefixed modules it consumes; T1 owns the resolver semantics. They meet at
the existing `dependency_modules(root) -> Vec<WorkspaceModule>` contract — agree
not to change its shape. Parallel-safe: T2 touches `deps.rs` fetch/lock + tests,
T1 touches the model checker.

## Acceptance criteria (testable)
1. `cargo build -p pseudoscript` passes (CONFIRMED green in this spike, exit 0).
   `cargo test -p pseudoscript` should pass (8 existing deps unit tests; not
   re-run here). The brief's assumed compile break does not exist.
2. `pds add <url> --path <sub>` writes `[dependencies].<name> = { git, path }`
   and a `pds.lock` `[[package]]` with a 40-char `rev`.
3. After `add`, only `<sub>` (not sibling subdirs) is materialised under
   `pds_modules/<slug>` — asserted by a `file://` fixture with a decoy sibling.
4. The checkout's `git rev-parse HEAD` == the locked `rev` (detached, pinned).
5. `pds install` on a fresh root with an existing `pds.lock` reproduces every
   package at its locked rev; a second `install` is a no-op.
6. `pds install` re-fetches (or bails clearly) when a `pds_modules` entry's HEAD
   no longer matches the lock (after step-2 hardening).
7. `pds add` against an unreachable url / bad ref / nonexistent `--path` exits
   non-zero with an actionable message and writes no `[dependencies]` entry.
8. `dependency_modules` exposes each direct dep's modules FQN-prefixed with the
   dep name; `pds doc`/checking resolve `dep::module::Node`.
9. `pds_modules/` is gitignored.

## Rough size (S/M/L) + parallel-safe?
**S–M.** The feature is built and wired; remaining work is one integration test
(the bulk), an `install` HEAD-verify guard, `--path` sanitisation, and a
spec/ADR consistency pass — all contained to `deps.rs` + a new `tests/deps.rs` +
docs. The only schedule risk is confirming the build/tests green (unverified
here). Parallel-safe with T1: distinct layers, stable `dependency_modules`
contract.
