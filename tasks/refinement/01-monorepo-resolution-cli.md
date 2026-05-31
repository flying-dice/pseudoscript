# T1 — Monorepo tools & cross-project resolution (#cli)

## Summary (1-2 lines)
Let one repo hold several `pds` workspaces and resolve `use`/FQN references across **sibling local projects** declared as path dependencies, plus discovery and aggregate (`check`/`doc`) commands over the set. Git dependencies (§8.4–§8.5) already work end-to-end; this task adds the *local path* dependency variant and monorepo orchestration on top of that machinery.

## Current state (with file:line refs)
- The CLI crate is `crates/pseudoscript` (binary `pds`). Subcommands: `init`, `lsp`, `check`, `fmt`, `tokens`, `doc`, `upgrade`, `add`, `install` — `crates/pseudoscript/src/main.rs:42-119`. Each operates on **one** workspace/file; there is no aggregate or multi-project mode.
- A workspace = one dir with a `pds.toml`. `find_root` walks up to the nearest manifest (`workspace.rs:79-95`); `load` parses `[doc]`, walks the tree for `.pds` modules, and also loads dependency modules — `crates/pseudoscript/src/workspace.rs:104-114`. The walker skips `target`, `pds_modules`, and dotdirs — `workspace.rs:232-241`.
- **Git deps are fully implemented**, not stubs (CLAUDE.md is stale here). `[dependencies]` entries carry `git` + revision selector + in-repo `path` — `crates/pseudoscript/src/deps.rs:38-48`. `pds add`/`pds install` resolve the transitive graph (cycle detection at `deps.rs:404-406`), write `pds.lock`, and sparse/blobless-checkout each package into `pds_modules/` — `deps.rs:220-250`, `:478-517`. ADR-024 and `LANG.md §8.4–§8.5` pin this.
- Cross-workspace resolution into the model already exists. `deps::dependency_modules` reads `pds.lock`, loads each **direct** dependency's modules, and prefixes every FQN with the dep name (`auth::core`) — `deps.rs:302-337`. `workspace::load` puts these in `Workspace.dependencies` — `workspace.rs:35,107`. `build_site` feeds them to `check_workspace_with_externals(&modules, &dependencies)` — `main.rs:389-392`.
- The model resolver already separates **local** (checked) from **external** (indexed-only) modules: `Workspace::build_with_externals` — `crates/pseudoscript-model/src/model.rs:380-404`; visibility enforced uniformly by `resolve_qualified` (`model.rs:442-452`); a private dep target → `Private` (rejected), a missing one in a known module → dangling. Tests at `crates/pseudoscript-model/src/lib.rs` cover public-resolves / private-rejected / dangling-rejected.
- **No local/path dependency variant.** `DepSpec` requires `git` (`deps.rs:39-48`); there is no `path = "../sibling"` form. So sibling projects in a monorepo cannot reference each other except by publishing to git and re-vendoring through `pds_modules/`.
- **No project discovery / aggregate tooling.** Nothing enumerates the `pds.toml` files under a root or runs `check`/`doc` across them. (The repo *is* a de-facto monorepo today — `pseudoscript/`, `examples/ticketing/`, `examples/patterns/` are separate workspaces — but the CLI only ever loads one.)
- No `DuplicateModule`/duplicate-FQN diagnostic exists; the global FQN index is a map (last write wins, `model.rs:387-390,398-401`), so a same-named module across the closure silently shadows.

## Proposed approach
Reuse the external-module machinery; add the missing path-dep variant and orchestration. Smallest viable design:

1. **Local path dependency.** Add a `path` field to `DepSpec` and make `git`/`path` mutually exclusive (untagged enum or validation in `selector`/`sub_path`). A path dep resolves to a sibling workspace dir relative to the **declaring manifest**: `banking = { path = "../banking" }`. No fetch, no `pds_modules/` entry — it is loaded in place.
2. **Load path deps into the model.** In `deps::dependency_modules` (or a sibling fn), for each direct path dep, resolve its dir, `workspace::load_modules` it, and emit dep-name-prefixed `WorkspaceModule`s exactly as git deps do (`deps.rs:329-334`). They flow into `Workspace.dependencies` and `check_workspace_with_externals` unchanged — cross-project resolution then works for free under the existing §8.2/§8.4 rules. Path deps need lockfile handling decided (likely recorded as a `path` package kind, or excluded from the lock since they are not pinned to a commit).
3. **Transitive + safety.** Walk path deps transitively (a path dep may itself declare path deps), dedup by canonicalized dir, and detect cycles — mirror the git resolver's `stack` check (`deps.rs:404-406`).
4. **Discovery + aggregate commands.** Add `pds list [ROOT]` (enumerate every `pds.toml` under ROOT, excluding `target`/`pds_modules`/dotdirs — reuse the `is_visible` walker shape, `workspace.rs:232-241`) and an `--all`/aggregate mode for `check` and `doc` that loads and processes each discovered project, exiting non-zero if any fails. MVP keeps per-project resolution isolated (each project + its declared deps); a single fused monorepo graph is deferred.

ADR note: a path dep is a §8.4 dependency whose source is a local path instead of git; its identity has no commit, so §8.5 lockfile pinning does not apply. Worth an ADR amending §8.4–§8.5 (path source) — co-author with the spec-style skill.

## Affected/new files
- `crates/pseudoscript/src/deps.rs` — add `path` source to `DepSpec`; mutually-exclusive git/path; load path-dep modules (extend or split `dependency_modules`); transitive walk + cycle/dedup for path deps; decide lock representation.
- `crates/pseudoscript/src/workspace.rs` — already exposes `load_modules` (reused); minor: resolve path-dep dirs relative to the manifest.
- `crates/pseudoscript/src/main.rs` — `pds list` subcommand; `--all`/aggregate flag for `check`/`doc`; an aggregate driver loop.
- New `crates/pseudoscript/src/monorepo.rs` (or `discover.rs`) — `pds.toml` discovery + path-dep graph walk.
- `crates/pseudoscript-model/` — likely **unchanged** (externals path already supports this); optionally add a duplicate-FQN diagnostic if uniqueness is enforced.
- Docs: `LANG.md §8.4–§8.5` (path source clause), new `decisions/NNN-path-dependencies.md`, `PATTERNS.md` monorepo recipe, CLI tests under `crates/pseudoscript/tests/`.

## Open questions / decisions needed
- Path-dep lockfile: record path deps in `pds.lock` (no commit → what identity?) or exclude them? Recommend exclude — they are not reproducible artifacts; resolve fresh each build.
- Strict vs fused monorepo graph: must a project declare a `[dependencies]` entry to see a sibling (strict, matches §8.4's explicit-root rule), or auto-resolve any sibling under the root (fused)? Recommend strict — preserves the §8.1/§8.4 "each root is declared" discipline.
- Duplicate module FQN across the closure: silently shadow (today), warn, or error? Recommend a diagnostic; currently last-write-wins in `by_fqn`.
- Root orchestration manifest (a monorepo-root `pds.toml` listing members) vs pure filesystem discovery for `list`/`--all`. Recommend discovery for MVP; manifest later.
- Should a path dep's own modules be checked (it is local, unlike a vendored git dep)? Today externals are not checked (`model.rs:392-402`). For a sibling in the same repo, checking it via its own `pds check` (aggregate mode) is cleaner than checking it as a consumer's external.
- Security/escaping: a `path` that escapes the repo root (`../../etc`) — constrain to within a discovered monorepo root?

## Dependencies on other tasks (T2 git deps especially)
- **T2 (git deps)** is effectively already landed in `deps.rs` — `pds add`/`install`, `pds.lock`, `pds_modules/`, transitive resolution, and the model's external-module support all exist. T1 *reuses* that machinery rather than blocking on it. The one real overlap: both extend `DepSpec`/`Dependency` and `dependency_modules`. If T2 has remaining polish, co-design the `DepSpec` source enum (git | path) once so both variants share the loader. Lower conflict risk than originally feared since the wiring is in place.

## Acceptance criteria (bullet, testable)
- `pds.toml` accepts `dep = { path = "../sibling" }`; `git` and `path` together is rejected with a clear error.
- `pds check` (or `pds doc`) on project A resolves a reference to a `public` node in sibling project B via a path dep; a reference to a **private** B node is rejected (extends §8.2/§8.4).
- A reference into a non-declared sibling is dangling/rejected (strict mode).
- Path deps resolve relative to the declaring manifest, not the CWD; an escaping path is rejected (if constraint adopted).
- A path-dep cycle is detected and reported, not hung/overflowed.
- `pds list <root>` enumerates every `pds.toml` under root, excluding `target`/`pds_modules`/dotdirs.
- `pds check --all <root>` checks every discovered project and exits non-zero iff any fails.

## Rough size (S/M/L) + parallel-safe? (which tasks it conflicts with)
**M.** Path-dep variant + model reuse is S (the externals path already exists); discovery + aggregate commands add M. **Conflicts only with T2** on `deps.rs` (`DepSpec` source enum) and the `dependency_modules` loader — coordinate the source-enum shape. Safe to parallelize against tasks not touching `deps.rs`/`workspace.rs`/CLI subcommands.
