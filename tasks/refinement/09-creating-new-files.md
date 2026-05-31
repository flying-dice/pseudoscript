# T9 — Creating new files (#web-ide)

## Summary

Add a "new file" action to the web-IDE so a user can create a `.pds` (or other general)
file in the open workspace: prompt for a name, validate it, seed a minimal `.pds`
skeleton, insert it into the in-memory `workspace.files` list, persist it to disk when
the workspace is folder-backed (File System Access API), open it in the editor, and let
the existing reactive resolution pick it up. Markdown/doc-page creation is T10; FS
lifecycle (open/save folder, handles, permissions) is T11; project init is T3.

## Current state (file:line)

Layout: `web-ide/` is a SvelteKit app. Single route `web-ide/src/routes/+page.svelte`
owns all workspace state; components live in `web-ide/src/lib/components/`; the wasm
wrapper is `web-ide/src/lib/pds.js`; FS helpers `web-ide/src/lib/workspace.js`; recents
`web-ide/src/lib/recents.js`; bundled examples `web-ide/src/lib/samples.js` +
`web-ide/src/lib/samples/`.

**Workspace data model** (`+page.svelte`):
- State: `workspace` (`+page.svelte:90`), `openFile` (`:91`), `moduleSources` (`:92`,
  an `{ [fqn]: sourceText }` map). The workspace is `{ name, root, base, files, manifestToml, docs? }`;
  `files` is a flat array of `{ fqn, path, handle }` (`handle` is the FS Access file
  handle, or null for in-memory samples). `workspace.root` is the directory handle (null
  for samples) — the folder-backed vs in-memory discriminator, used throughout
  (`:482`, `:638`, `:688`).
- `allModules` (`:137`) maps `workspace.files` → `[{ fqn, source }]` from `moduleSources`.
- Resolution is fully reactive: `workspaceResults`/`problems` (`:141`,`:151`),
  `nodes`/`outlineModules` (`:170`), `structureByFile`/`outline` (`:181`), `symbols`
  (`:205`), `canvas` (`:239`) all derive from `allModules`. **Adding a file to
  `workspace.files` and seeding `moduleSources[fqn]` re-runs resolution automatically —
  no explicit re-resolve call exists or is needed.**

**Persistence** (`workspace.js`):
- `scheduleSave(handle, text)` (`+page.svelte:559`) debounces `writeFile` 400ms; it
  no-ops when `handle` is null (in-memory sample → session-only). `onEditorChange`
  (`:565`) routes edits to `moduleSources` (or `docSources`) and calls
  `scheduleSave(openFile.handle, value)`.
- `workspace.js` wraps the FS Access API: `fsSupported = 'showDirectoryPicker' in window`;
  `openWorkspace()` → `showDirectoryPicker()`; `readWorkspace(root)` walks the tree
  collecting `.pds` files as `{ fqn, path, handle }`; `readFile(handle)`;
  `writeFile(handle, text)` via `createWritable()`; `writeSite(root, files)` creates the
  output subdir + files via `getDirectoryHandle({ create: true })` /
  `getFileHandle({ create: true })`. **The create-file primitives
  (`getFileHandle({create:true})` + `createWritable`) already exist inside
  `writeFile`/`writeSite`, but there is no single `createFile(root, path)` export** — that
  is the natural new helper for T9.
- `fqn` is derived from `path` the same way in `readWorkspace` and `samples.js` (path with
  the `.pds` extension dropped, segments joined as the module FQN) — a create handler must
  reuse that exact derivation so the new file's `fqn` matches its `path`.

**Opening a created file**: `selectFile(file)` (`+page.svelte:577`) sets `openFile` and
switches to code view — the create handler reuses this.

**FileTree** (`FileTree.svelte`): props `workspaceName, files, openPath, docGroups,
symbols, selectedFqn, errorPaths` + callbacks `onopen, ondocopen, onpicknode`
(`+page.svelte:864-875`). Three sections: a **Documentation** list from `docGroups`
(`FileTree.svelte:69-92`); a **flat Files list** — `{#each files as file}` rendering one
button per `file`, labelled by `file.fqn`, no folder grouping (`:94-117`); and a
**Symbols** tree that *is* nested, built by `parent`-linking the flat `symbols` array into
a hierarchy and rendered recursively via the `row` snippet (`:43-60`, `:119-162`). **It is
read+navigate only — no create / add / rename / delete / context-menu affordance, and no
filesystem-path tree.**

**No create affordance exists anywhere today** — `+page.svelte` has open-folder /
open-sample / build-docs / format, but no add-file path. New files only arrive via
`openFolder` / `openSample` / `openRecent`. `samples.js` (`SAMPLES` + `loadSample(id)`)
imports sample `.pds` as raw strings (Vite glob), all with `handle: null`.

## Proposed approach

1. **Affordance** — "New file" (+) button on the FileTree's Files section header. The
   Files list is flat (FQN-labelled), so there's no folder row to right-click — placement
   is a name/path the user types (default workspace root). Emits a new `oncreatefile`
   callback up to `+page.svelte`.
2. **Name prompt + validation** — small dialog (or inline input). Validate: non-empty,
   no path separators in the leaf, no collision with an existing `file.path`, reserved
   names rejected (`pds.toml`). Append `.pds` when no extension. In T9 reject `.md`
   (owned by T10).
3. **Skeleton** — seed a minimal valid `.pds` module so the new file doesn't immediately
   error. Confirm the exact minimal module the checker accepts (likely a
   `system "<Name>" { }` shell) from `LANG.md`/the canonical sample, or validate a
   candidate via `check()`/`outline()` from `pds.js`.
4. **In-memory add** — push `{ fqn, path, handle }` into `workspace.files`, reassigning
   for reactivity (`workspace = { ...workspace, files: [...workspace.files, newFile] }`),
   and set `moduleSources[fqn]` to the skeleton. Derive `fqn` with the same path→fqn rule
   `readWorkspace`/`samples.js` use.
5. **Disk persist (folder-backed only)** — if `workspace.root`, create the file on disk
   and stash its `handle` on the new file entry so later edits save through the existing
   `scheduleSave` path. Add a `createFile(root, path)` export to `workspace.js` reusing the
   `getDirectoryHandle({create:true})` / `getFileHandle({create:true})` +
   `createWritable` pattern already in `writeSite` (overlaps T11). In-memory sample →
   `handle: null`, skip disk (matches `scheduleSave`'s null-handle no-op).
6. **Re-resolution** — none required; the reactive `$derived` graph keyed on `allModules`
   re-runs on the `workspace.files` + `moduleSources` mutation. Then `selectFile(newFile)`.

## Affected/new files

- `web-ide/src/routes/+page.svelte` — own the create handler (validate, mutate
  `workspace.files` + `moduleSources`, persist, `selectFile`); pass `oncreatefile` to
  FileTree. **(conflicts with T10/T11)**
- `web-ide/src/lib/components/FileTree.svelte` — add the "+"/context-menu affordance and
  emit `oncreatefile`. **(conflicts with T10/T11)**
- `web-ide/src/lib/workspace.js` — add a `createFile(rootHandle, path)` export reusing the
  existing `getDirectoryHandle({create:true})` / `getFileHandle({create:true})` +
  `createWritable` pattern (overlaps T11).
- New `web-ide/src/lib/skeletons.js` (or a const in `+page.svelte`) — the `.pds` template.
- Optional new `web-ide/src/lib/components/NewFileDialog.svelte` — name prompt/validation,
  shared with T10.

## Open questions / decisions needed

1. Minimal valid `.pds` skeleton the checker accepts without errors.
2. Confirm the precise path→fqn rule in `readWorkspace`/`samples.js` so the created file's
   `fqn` matches (the derivation exists; just transcribe it exactly).
3. Naming policy: auto-append `.pds`; in T9 allow `.pds` + extensionless general files,
   reject `.md`? Confirm scope with backlog owner ("general files" is in the title).
4. Disk write timing: immediate on create vs first edit (T3 save). Immediate is clearer
   but couples to T11's handle lifecycle.
5. In-memory workspace (no `root`): allow create (memory only, session-only) or disable
   the affordance?
6. Collision UX: reject vs auto-suffix (`module-1.pds`).
7. Non-root placement: the Files list is flat (no folder rows), so a subdirectory target
   must come from the typed name (e.g. `banking/core`) rather than a folder context-menu —
   confirm that's acceptable for T9, and that validation should then permit `/` as a path
   separator (vs the leaf-name no-separator rule above).

## Dependencies on other tasks

- **T3 (init/save)** — shares the disk-write path; if T3 adds a generic write helper, T9
  should call it. Co-design or sequence T3 first.
- **T10 (doc/markdown files)** — sibling; T9 excludes `.md`. Share the NewFileDialog and
  the create-handler shape; extract once to avoid duplicate conflicting edits.
- **T11 (FS management)** — owns directory handles, permissions, the FS Access wrapper in
  `workspace.js`. T9's persist step depends on a `createFile` helper landing there.
  High conflict risk in `workspace.js`. Co-design the handle/create API.
- **T13 (sync)** — if T13 reconciles disk↔memory, a created file must register with it so
  it isn't seen as an external add or clobbered.

## Acceptance criteria (testable)

1. "New file" creates a file at the typed path (default workspace root); a path with `/`
   segments places it in (and creates) the named subdirectory.
2. A name with no extension gets `.pds`; the new module appears in the tree and opens in
   the editor.
3. Collision with an existing path is rejected with a visible message; no mutation.
4. Names with `/` or `\`, empty names, and reserved names (`pds.toml`) are rejected.
5. Creating a `.md` file is rejected/redirected (owned by T10).
6. A new `.pds` skeleton resolves clean (no spurious diagnostics for it on creation).
7. Folder-backed: the file exists on disk after creation (verifiable by reopening).
8. After creation the reactive graph re-runs and the new module is referenceable from
   other modules (cross-module resolution sees it).
9. In-memory sample: create succeeds in memory, no disk write attempted (no error toast).

## Rough size (S/M/L) + parallel-safe?

**M.** UI affordance + dialog + a `workspace.files`/`moduleSources` mutation, reusing the
existing reactive resolution and `selectFile`. The only real new plumbing is the disk
`createFile` helper. Grows toward L if that helper must be built here rather than reused
from T3/T11.

**Not parallel-safe.** `FileTree.svelte` and `+page.svelte` collide with **T10** (shared
affordance + dialog) and `workspace.js` collides with **T11** (FS wrapper). Recommend
landing the shared NewFileDialog + create-handler first, or serialize T9 → T10, and
co-design the `createFile` API with T11.
