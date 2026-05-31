# T3 — Init new workspace + save to disk (#web-ide)

## Summary
Add a "New workspace" path that lets a user create a brand-new, templated PseudoScript
workspace (a `pds.toml` + one starter `.pds` module, optionally a `docs/` page) and
persist it to a directory on their disk via the File System Access API directory
handle — so it behaves exactly like an opened folder (edits autosave, docs build to
`target/doc/`), not a session-only in-memory sample.

Today the IDE can only *open* an existing folder or *load a read-only sample*. There is
no way to author a new project from scratch and have it live on disk. This closes that
gap with the smallest possible surface: pick an empty directory, scaffold the canonical
files into it, then re-enter the existing `readWorkspace` flow.

## Current state (file:line)
Two workspace origins exist, both feeding the same in-memory shape
`{ name, root, base, files:[{path,fqn,handle}], manifestToml, docs? }`:

- Open existing folder — `web-ide/src/lib/workspace.js:34` `openWorkspace()` calls
  `window.showDirectoryPicker({ mode: "readwrite" })` then `readWorkspace(root)`
  (`workspace.js:45`), which walks (`workspace.js:60`) for `.pds` files and reads
  `pds.toml` (`workspace.js:46`). Wired in `+page.svelte:605` `openFolder()`.
- Load sample — `samples.js:78` `loadSample(id)` returns the same shape with
  `root: null` and `handle: null` per file (in-memory; edits session-only). Wired in
  `+page.svelte:499` `openSample()`.

Disk writes already exist and are reusable:
- `writeFile(handle, text)` — `workspace.js:123` (createWritable/write/close).
- `writeSite(root, files)` + `mkdirp(root, parts)` — `workspace.js:133,147`
  (creates nested dirs with `{ create: true }`, used by `runBuild` at `+page.svelte:683`).
- Per-edit autosave — `scheduleSave(handle, text)` at `+page.svelte:559`; no-ops when
  `handle` is null (the sample case), debounced 400ms to `writeFile`.

Mounting + recents:
- `mountWorkspace(ws, landing)` — `+page.svelte:453` swaps in a workspace, resets nav,
  loads docs (`loadWorkspaceDocs`, `+page.svelte:473`).
- `recordFolder(name, rootHandle)` — `recents.js:49` persists the dir handle to
  IndexedDB so it re-opens later; `reopenFolder` (`recents.js:65`) re-grants permission.
- Manifest name scanned by `workspaceNameFrom` (`workspace.js:14`); `[workspace] entry`
  (e.g. `outbox.pds`) is the convention but is currently only authored, not read by the
  loader — the landing module comes from `meta.json` for samples and `files[0]` for
  folders.

Entry point UI: `ProjectPanel.svelte` "Open a folder…" button (`ProjectPanel.svelte:90`,
fired via `onopenfolder`). `Toolbar.svelte` has a project button (`+page.svelte:850`).
`canOpenFolder={fsSupported}` (`workspace.js:7`) gates folder features to Chromium.

**Gap:** no function scaffolds files into a chosen directory, and no UI invokes one.
Everything needed to *write* and *mount* already exists.

## Proposed approach
Add one library function + one UI affordance; reuse the rest.

1. **`workspace.js` → `createWorkspace(name)`** (new):
   - `const root = await window.showDirectoryPicker({ mode: "readwrite" })` — user
     picks the *parent/target* directory (the new workspace lives directly in it; the
     dir name doubles as a natural workspace name fallback).
   - Guard against clobber: if `pds.toml` already exists in the picked dir, reject with a
     typed error (`"not-empty"`) so the caller can prompt ("this folder already has a
     workspace — open it instead?"). Optionally also flag if any `.pds` exists.
   - Write the template via existing `writeFile` + `getFileHandle(..., {create:true})`:
     `pds.toml`, a starter module (default `main.pds`), and `docs/overview.md`.
   - Return `readWorkspace(root)` so the result is byte-identical to an opened folder
     (handles populated, autosave live).
2. **Template** (inline consts in `workspace.js`, mirroring the
   `WORKED_MODEL`-style stable-const convention, not the sample folders):
   - `pds.toml`: `[workspace] name/entry=main.pds`, `[doc] name/theme="dark"`, one
     `[[doc.sidebar]]` group pointing at `docs/overview.md`.
   - `main.pds`: minimal valid module — `module main` + one `system` with a
     `description`. Must pass `checkModules` clean (verify against the static checker).
   - `docs/overview.md`: a one-line `# <name>` placeholder.
3. **UI**: add a "New workspace" button in `ProjectPanel.svelte` beside
   "Open a folder…" (`ProjectPanel.svelte:90`), gated by `canOpenFolder`. New prop
   `onnewworkspace`. A lightweight name prompt (inline text field or a tiny modal —
   reuse the `.modal`/`.scrim` styling already in `+page.svelte:1104`).
4. **Wire in `+page.svelte`**: `newWorkspace()` mirrors `openFolder()`
   (`+page.svelte:605`): call `createWorkspace(name)`, populate `moduleSources` from the
   read files, `mountWorkspace(ws, ws.files[0]?.fqn)`, `recordFolder(ws.name, ws.root)`,
   `refreshRecents()`, `flash("Created …")`. Handle the `not-empty` rejection with a
   `notify("error", …)`.

## Affected/new files
- `web-ide/src/lib/workspace.js` — **new** `createWorkspace(name)` + template consts;
  reuses `writeFile`/`readWorkspace`. (~40 lines)
- `web-ide/src/lib/components/ProjectPanel.svelte` — new "New workspace" button +
  `onnewworkspace` prop + optional name field.
- `web-ide/src/routes/+page.svelte` — new `newWorkspace()` handler; pass
  `onnewworkspace={newWorkspace}` to `<ProjectPanel>` (`+page.svelte:736`).
- (No wasm change — no scaffold export exists in `pds-wasm`, and none is needed; the
  template is static text validated by the existing `checkModules`.)

## Open questions / decisions needed
- **Target-dir semantics**: write the workspace *into* the picked dir (simplest, matches
  `writeSite`) vs. create a *named subdirectory* inside it (needs `getDirectoryHandle(name,
  {create:true})` and a slugified name). Recommend: write into the picked dir; the dir is
  the workspace. Decide before build.
- **Name source**: prompt for a name, or derive from the picked dir name
  (`root.name`)? Recommend deriving with an editable default to keep zero-modal.
- **Starter module filename**: `main.pds` vs `context.pds` (samples use varied names;
  `entry` makes it explicit). Recommend `main.pds`.
- **Template breadth**: bare `system` only, vs. a richer person+system+container starter
  that better demonstrates the language. Recommend minimal-but-valid; richer templates
  are a follow-up (could become a "template picker").
- **Non-empty dir UX**: hard error vs. offer to open the existing workspace. Recommend a
  typed rejection the UI turns into an "Open it instead?" notification.
- **Permission durability**: `showDirectoryPicker` grants the session; `recordFolder`
  already persists the handle for re-open — confirm the grant covers the immediate
  writes (it does, `mode:"readwrite"`).

## Dependencies on other tasks
- **T7 (import/export)**: shares the "materialise a workspace from a template/source"
  concept; `createWorkspace` should be factored so an import path can reuse the
  write-then-`readWorkspace` flow. Coordinate the file-writing helper, but T3 doesn't
  block on T7.
- **T9 (new files)**: T3 scaffolds the *initial* file set; T9 adds files to an existing
  workspace. Both want a shared "write a new `.pds`/doc into the root and refresh the
  tree" helper — design that helper here so T9 inherits it. Mild overlap, not blocking.
- **T11 (FS mgmt)**: rename/delete/move build on the same directory-handle primitives;
  T3 only adds *create*. Independent; T11 may later absorb the clobber-guard logic.
- **T13 (sync indicator)**: a created workspace has a live handle, so the same
  saved/dirty indicator applies with no extra work; T3 should leave a hook (it already
  flows through `scheduleSave`). Independent.

## Acceptance criteria (testable)
1. In a Chromium browser, "New workspace" in the project panel prompts for a directory;
   on confirm, the picked dir contains `pds.toml`, `main.pds` (or chosen entry), and
   `docs/overview.md` on disk.
2. The new workspace mounts immediately with `workspace.root` set (not null), files
   listed in the FileTree, and the starter module open.
3. The starter `main.pds` checks clean — `checkModules` returns zero error diagnostics
   (Problems tab shows 0 errors).
4. Editing the starter module autosaves to disk (verify via `scheduleSave` → file
   contents change on disk after the 400ms debounce).
5. "Build docs" on the new workspace writes `target/doc/` to disk (not the
   preview-modal path), because `workspace.root` is set.
6. The new workspace appears in Recent and re-opens via `reopenFolder` after a reload
   (subject to permission re-grant).
7. Picking a directory that already contains `pds.toml` does **not** overwrite it; the
   user gets a clear "already a workspace" message.
8. On a non-Chromium browser the "New workspace" control is disabled/hidden (parity with
   "Open a folder…", gated by `fsSupported`).

## Rough size (S/M/L) + parallel-safe?
**S–M.** One new ~40-line `workspace.js` function (reusing `writeFile`/`readWorkspace`),
one button + handler, a static template. Largest unknowns are UX decisions (name prompt,
non-empty handling), not engineering. **Parallel-safe**: touches files (`workspace.js`,
`ProjectPanel.svelte`, `+page.svelte`) that T7/T9/T11 also touch, so it conflicts at the
merge level — sequence the shared file-write helper with T9, but T3 can be built
independently against the current tree.
