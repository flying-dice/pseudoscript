# T13 — Workspace sync/save indicator (#web-ide)

## Summary
Surface save state in the web IDE: a per-file dirty marker (unsaved dot on the
file-tree row and the active-file status segment), a workspace-level "all saved /
N unsaved" indicator in the status bar, and explicit save feedback (Cmd/Ctrl-S +
"saving…/saved" status). Today the IDE silently debounce-writes edits to disk and
shows the user nothing about whether their work is persisted — or whether it can
be persisted at all (bundled samples are session-only). T13 adds the missing
"dirty vs persisted" notion and renders it.

## Current state (file:line) — existing dirty/save tracking
All state lives in `web-ide/src/routes/+page.svelte`. There is **no** dirty or
persisted tracking today — only a fire-and-forget debounced disk write.

- `moduleSources` / `docSources` (`+page.svelte:92`, `:97`) hold the live in-memory
  edit buffers, keyed by file FQN (modules) and path (docs). These are the
  "current content".
- `onEditorChange(value)` (`:565`) writes the buffer and calls
  `scheduleSave(openFile.handle, value)` — no comparison against any baseline, no
  flag set.
- `scheduleSave(handle, text)` (`:559`): `if (!handle) return;` then a **400 ms
  debounced** `writeFile(handle, text)`; on failure it `flash("Could not save to
  disk")`. Bundled samples have **no `handle`** → never persisted ("in-memory
  sample: session-only", `:560`).
- `saveTimer` is the single shared debounce handle. It is **cleared on every file
  switch** without flushing: `applyLocation` (`:71`), `selectNode` (`:378`),
  `mountWorkspace` (`:455`), `selectFile` (`:577`), `openDoc` (`:588`). A pending
  edit made <400 ms before switching files is therefore dropped from disk →
  silent data loss, and the user has no dirty cue. (Worth fixing under T13.)
- Status bar (`:929`–`:940`) shows: `pds`, `wasm <ver>`, open-file FQN, module
  count, transient `toast`, current `view`, current scope. **No save state.**
- `flash(message)` (`:541`) is a 2.4 s transient status toast (distinct from the
  `notify` notification stack at `:550`). Disk-write success is currently silent;
  only failure flashes.
- Persistence layer is `web-ide/src/lib/workspace.js`
  (`writeFile`, `writeSite`, `openWorkspace`, `readWorkspace`, `readFile`,
  `readDocPages`) over the File System Access API; `fsSupported` gates folder
  open. Files carry an FS `handle`; samples do not.
- `FileTree.svelte` already renders an **error** marker per path via the
  `errorPaths` prop (`+page.svelte:159`, passed at `:872`) — the same row-decoration
  mechanism a dirty dot should reuse. FileTree renders it as
  `class:has-error={errorPaths.has(file.path)}` (`FileTree.svelte:105`) with a
  `::after` dot (`FileTree.svelte:262`) — mirror this for the dirty marker.
- `Editor.svelte` has a `shortcutRun` command map (`Editor.svelte:524`) on a keymap
  compartment (`:534`), chords from `keybindings.svelte.js` (e.g.
  `openSettings: "Mod-k Mod-s"`, `keybindings.svelte.js:41`). A save command drops
  straight into this catalogue + map.
- `Toolbar.svelte` already takes `errorCount` / `workspaceName` / `building`
  (`Toolbar.svelte:2`) and renders a status pill (`:45`) — a save indicator slots
  beside it.

Conclusion: dirty tracking must be built from scratch. The natural baseline is
"content last read from / written to disk".

## Proposed approach (dirty model + UI + save flow)

### Dirty model
Track a **persisted baseline** per file and derive dirty by comparison.
- Add `persisted = $state({})` keyed by the same keys as `moduleSources` /
  `docSources` (FQN for modules, path for docs). Seed it whenever content comes
  from or goes to disk:
  - on `openFolder` / `openRecent` after `readFile` (`:609`, `:523`) — seed each
    file's baseline to its on-disk text;
  - on a successful `writeFile` — set `persisted[key] = text`.
- Derive `dirty` as a `$derived` set: a file is dirty iff
  `current(key) !== persisted[key]`. Prefer **direct string compare** over a hash
  — model files are small (single .pds modules / Markdown pages), the comparison is
  cheap, and a hash adds complexity with no real win. (Revisit only if profiling a
  huge workspace shows cost.)
- Samples (no `handle`): they have **no baseline and cannot be saved**, so they are
  never "dirty" in the disk sense. Show them as "session-only / not saved to disk"
  rather than a dirty dot, to avoid implying a save action that does nothing.
- `dirtyCount = $derived(dirty.size)` drives the workspace-level indicator.

### UI affordances
- **File-tree row dot**: pass a `dirtyPaths` set into `FileTree` alongside the
  existing `errorPaths`, render a small unsaved dot (precedence: error marker wins
  visually over dirty, or show both). Reuse the existing per-row decoration slot.
- **Active-file status segment**: in the status bar, append a "●"/"saved" state to
  the open-file FQN segment (`+page.svelte:933`).
- **Workspace indicator**: a new status-bar segment — "All saved" when
  `dirtyCount === 0`, "N unsaved" otherwise; "session only" for a sample workspace.
  Optionally also a Toolbar affordance (Toolbar already takes `workspaceName`,
  `building`, etc. at `+page.svelte:846`).
- **Transient save feedback**: replace the silent success path in `scheduleSave`
  with a "saving…" → "saved" status via the existing `flash`/status segment.

### Save flow
- **Autosave (keep)**: retain the 400 ms debounce, but **flush on file switch**
  instead of dropping it — before clearing `saveTimer`, if a pending write exists,
  await/trigger it. This closes the current silent-loss gap and keeps dirty/saved
  honest.
- **Manual save (add Cmd/Ctrl-S)**: add a `saveDocument` entry to `shortcutRun`
  (`Editor.svelte:524`) with a chord in `keybindings.svelte.js`, forwarding a new
  `onsave` prop (mirroring `onformat` at `Editor.svelte:529`). Add a
  `svelte:window` fallback at `+page.svelte:724` so save works with no editor
  focus (e.g. on the canvas/problems view). The handler flushes the active file
  immediately and clears its dirty state.
- **beforeunload guard**: when `dirtyCount > 0` and any file lacks a handle (or a
  write is in flight), warn before tab close.

### Decision lean
Keep autosave as the default (it exists and users rely on it), add Cmd/Ctrl-S as
an explicit flush + reassurance, and make the indicator the primary feedback. This
is the smallest change that makes persistence legible without changing behaviour.

## Affected/new files
- `web-ide/src/routes/+page.svelte` — `persisted` state + seeding; `dirty` /
  `dirtyCount` deriveds; flush-on-switch in `scheduleSave`/`saveTimer` clears;
  `onsave` handler + window keybind; status-bar segments; pass `dirtyPaths` to
  FileTree. (primary)
- `web-ide/src/lib/components/FileTree.svelte` — accept `dirtyPaths`, render the
  unsaved dot per row (mirror `errorPaths`).
- `web-ide/src/lib/components/Toolbar.svelte` — optional workspace save indicator.
- `web-ide/src/lib/components/Editor.svelte` — add a save command to the keymap,
  forward `onsave` (mirror existing `onformat`).
- `web-ide/src/lib/keybindings.svelte.js` — register the Cmd/Ctrl-S binding (the
  Settings modal lists shortcuts; keep it discoverable there).
- `web-ide/src/lib/workspace.js` — no change expected (writeFile already returns a
  promise the new flush can await).

## Open questions / decisions needed (autosave vs manual?)
- **Autosave vs manual default?** Lean: keep autosave + add manual Cmd/Ctrl-S flush.
  Confirm we don't want a "manual-only" mode toggle in Settings.
- **Debounce window** — keep 400 ms, or lengthen now that a dot shows in-flight
  state? Flush-on-switch matters more than the exact value.
- **Sample workspaces** — show "session only" (no dirty dot) vs show a dirty dot
  that offers "Save to a folder…"? Lean: "session only" label, with the existing
  build-notice/open-folder path as the escalation.
- **Dirty after disk drift** — if a file changes on disk out of band, the baseline
  is stale. Out of scope for T13 (no file watching today); note as a follow-up.
- **Error+dirty precedence** in the tree row (both markers vs one).

## Dependencies on other tasks (T3, T9, T10, T11 — all write to disk)
Any task that writes a file to disk MUST update the persisted baseline so dirty
clears. Make baseline-seeding a single shared helper so each path calls it.
- **T3 (init/save)** — initial workspace creation / explicit save: seed/refresh
  baseline on create and on save.
- **T9 / T10** — any operation that mutates and persists file content must set
  `persisted[key]` on success (and mark dirty between edit and write).
- **T11 (FS management)** — create/rename/delete/move must keep `persisted` and the
  dirty/error sets keyed consistently (rename = re-key, delete = drop key).
This makes the baseline-update helper the integration contract for T13.

## Acceptance criteria (testable)
1. Editing a folder-workspace file marks it dirty (tree dot + active-file
   indicator) within one debounce window.
2. After a successful disk write (autosave or Cmd/Ctrl-S), the file's dirty marker
   clears and a "saved" cue shows briefly.
3. Workspace indicator reads "All saved" when no file is dirty and "N unsaved"
   when N files differ from their persisted baseline; count is exact.
4. Switching files with a pending edit does **not** lose the edit on disk (pending
   write is flushed, not dropped) — the current `clearTimeout(saveTimer)` data-loss
   path is closed.
5. Bundled samples (no handle) never show a disk dirty dot; they show a
   "session only / not saved to disk" state instead.
6. Cmd/Ctrl-S on a folder file flushes immediately and clears its dirty state; the
   binding appears in the Settings shortcuts list.
7. A failed disk write keeps the file dirty and surfaces the existing failure
   notification (no false "saved").
8. With unsaved work, closing the tab triggers a browser confirm.

## Rough size (S/M/L) + parallel-safe?
**M.** State + deriveds + flush-on-switch in `+page.svelte` are contained; the
risk is touching the same shared `saveTimer`/file-switch paths and the FileTree
row-rendering that T11 also edits. Parallel-safe with T3/T9/T10/T11 only if the
baseline-update helper lands first as a shared seam; otherwise serialize T13 after
T11 (FS management) to avoid colliding on FileTree row markup and key lifecycle.
