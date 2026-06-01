# Go-live backlog

Final push to ship the CLI + web-ide. 17 tasks. Per-task refinement specs (current state, file:line, approach, acceptance criteria) live in [`refinement/`](refinement/). This file is the map: sequencing, sizes, dependencies, and the decisions already settled.

Each task is meant to run in its own session/worktree. Pull anything marked **Ready**.

## Decisions settled (apply these — don't re-litigate)

| # | Decision | Source |
|---|----------|--------|
| Local path deps | A `[dependencies]` entry with `path` and no `git` is a local sibling workspace, resolved relative to `pds.toml`, **not** lock-pinned. `path` is overloaded: repo subdir under `git`, local dir without. | [ADR-026](../decisions/026-local-path-dependencies.md) |
| Share/export | **Client-only** — gzip the whole workspace into the URL hash; over ~2MB prompt file export. No server/KV. | T6/T7 |
| IDE theme | **Light + dark toggle** (+ `prefers-color-scheme` seed). T16 owns the `app.css` token pass and must define both ramps. | user |
| Doc default (T15) | **Explicit `meta.json` `landing` wins.** First-doc default applies only when no `landing` is set. ⚠️ shipped samples set `landing` to code FQNs → first-doc is inert for them until a content pass clears/redirects (folded into T14). | user |
| Mobile | **Dropped.** Not in scope. | user |

## Sequencing

```
#cli (independent track, parallel to all web-ide):
  T2 git-deps gaps ──┐ coordinate the DepSpec source enum (git|path), then parallel
  T1 path deps + monorepo discovery ──┘  (T1 unblocked by ADR-026)

web-ide foundation (do first — extracts shared helpers the cluster reuses):
  T13 dirty/baseline tracking + shared "write file → refresh tree → mark clean" helper
  T3  init workspace + save to disk (createWorkspace / disk write)

web-ide FileTree cluster (serialize — all touch FileTree.svelte / +page.svelte / workspace.js):
  T8 edit pds.toml ──▶ T10 new doc files (needs sidebar registration from T8)
  T9 new files ──▶ T11 move/rename/delete

web-ide independent (start anytime, isolated files):
  T4 doc preview   T5 search   T6+T7 (shared codec)   T12 md prettify   T14(+T15)   T16
```

## Tasks

| # | Title | Tag | Size | State | Depends on | Touches |
|---|-------|-----|------|-------|-----------|---------|
| T1 | Monorepo: local path deps + project discovery (`pds list`, `--all`) | cli | M | **Ready** | ADR-026; coord T2 enum | `deps.rs`, `main.rs`, `workspace.rs` |
| T2 | Git deps: integration test, verify HEAD==lock, sanitize `..`, `update`/`remove` | cli | S–M | **Ready** | coord T1 enum | `deps.rs` |
| T3 | Init new workspace + save to disk | web-ide | S–M | **Ready** | — | `workspace.js`, `ProjectPanel.svelte`, `+page.svelte` |
| T4 | Fix doc preview links/images; folder-only docs | web-ide | M | **Ready** | — | `markdown-live.js`, `workspace.js` |
| T5 | Search bar (in-file find via `@codemirror/search`; cross-file stretch) | web-ide | S | **Ready** | — | `Editor.svelte`, `package.json` |
| T6 | Compressed share URL (client-only, gzip hash, ≤~2MB) | web-ide | M | **Ready** | shared codec w/ T7 | new `lib/codec.js`, route, `Toolbar.svelte` |
| T7 | Import/export compressed workspace file | web-ide | M | **Ready** | shared codec w/ T6 | `lib/codec.js`, `Toolbar.svelte` |
| T8 | Editing pds.toml (raw TOML editor + re-resolve + validate) | web-ide | M | **Ready** | — (prereq for T10) | `workspace.js`, `+page.svelte`, FileTree |
| T9 | Creating new files (.pds) | web-ide | M | Blocked | foundation/shared helper | `FileTree.svelte`, `+page.svelte`, `workspace.js` |
| T10 | Creating new doc files (+ `[[doc.sidebar]]` registration) | web-ide | M | Blocked | T8, foundation | `FileTree.svelte`, `+page.svelte`, `workspace.js` |
| T11 | FS management: move / rename / delete | web-ide | L | Blocked | T9, T10 | `FileTree.svelte`, `workspace.js`, `recents.js` |
| T12 | Markdown source formatting (prettier standalone, branch `onformat`) | web-ide | M | **Ready** | — | `+page.svelte` |
| T13 | Workspace sync/save indicator (dirty model + UI + Cmd-S) | web-ide | M | **Ready** | foundation for cluster | `+page.svelte`, `FileTree.svelte`, `Toolbar.svelte` |
| T14 | 3–4 composed-pattern example workspaces (+ T15 sample-landing pass) | web-ide | L | **Ready** | `pseudocode` skill | `lib/samples/*` (zero-build glob) |
| T15 | Default to first doc when no `landing` set | web-ide | S | **Ready** | coord mount w/ T3/T6/T7 | `+page.svelte` |
| T16 | Logo + styling pass; light/dark toggle; token scales | web-ide | M–L | **Ready** | owns `app.css` tokens; `frontend-design` skill | `app.css`, `app.html`, `static/`, components |
| T17 | Cloudflare Workers Builds for web-ide + web-landing (prod-on-main + PR preview URLs) | ops | S | **Ready** | needs CF dashboard access | CF dashboard config — [spec](refinement/17-cloudflare-workers-builds.md) |

## Coordination notes

- **DepSpec source enum (T1↔T2):** agree the `git | path` shape and the `dependency_modules` loader contract before either ships. They're parallel once that's fixed.
- **Shared codec (T6↔T7):** one versioned gzip-JSON envelope `{ v, name, manifestToml, files[], docs[] }`. Build it once in `lib/codec.js`; T6 base64url-encodes the same bytes T7 downloads.
- **FileTree cluster (T9/T10/T11/T13):** extract a shared NewFileDialog + create/write/dirty helper in the foundation step (T13/T3), or these will conflict-thrash the same three files. Serialize: T9→T11, T8→T10.
- **Mount entry points (T15):** the first-doc rule must cover folder / sample / recent / share-import — keep the `docLoadSeq` race guard; `loadWorkspaceDocs` must return its groups so the choice isn't made before docs load.

## Done this session

ADR-026 written (`decisions/026-local-path-dependencies.md`), indexed in `decisions/README.md`, and `LANG.md` §8.4/§8.5 amended for the local source. Nothing committed (commit is yours / gated). No spec corruption — an earlier note about a corrupted §8.6 and a stale ADR index were both tooling read artifacts, since verified false.
