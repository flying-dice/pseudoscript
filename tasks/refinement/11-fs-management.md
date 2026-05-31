# T11 — FS management: move/rename/delete (#web-ide)

## Summary

The web-ide tree (`FileTree.svelte`) is a read-only view: it lists `.pds` files and the
model's symbol hierarchy, with no per-file actions at all — no rename, move, delete, or create.
Files are loaded from a real on-disk folder via the File System Access API (`workspace.js`), and
edits already persist back through a retained `FileSystemDirectoryHandle`. This task adds rename,
move (drag & drop between folders), delete, and new-folder, applied to **both** the in-memory
workspace (which drives wasm resolution) **and** the backing disk. The non-trivial part is
resolution: a file's **FQN is derived from its path** (base-relative, `.pds` dropped, `/`→`::`),
and that FQN is the key the whole model — diagnostics, diagrams, cross-module references — is built
on. Renaming or
moving a `.pds` therefore changes its module identity and can dangle any importer.

## Current state (file:line) — tree model + FS handle availability

**Tree data model — `FileTree.svelte`**
- Props: `workspaceName, files, openPath, onopen, onpicknode, errorPaths, docGroups, ondocopen,
  symbols, selectedFqn` (FileTree.svelte:14-31). The component is **purely presentational** — it
  only owns `collapsed` UI state for the symbol tree (FileTree.svelte:34); every action dispatches
  upward (`onopen`, `onpicknode`, `ondocopen`). There are **no** rename/move/delete/new affordances.
- `files` is a flat list of `{ path, fqn, handle }` (built in workspace.js:42-45; typedef at
  workspace.js:124-126 — note **no `name` field**). The Files section just renders each as a button
  showing `file.fqn` (FileTree.svelte:99-115). Folders are **not modelled at all** — the file list
  is flat and never re-nested into directories, so the tree shows the `::`-joined FQN as a flat
  label, not nested folders. (Contrast the Symbols section, which nests C4 nodes by structural
  `parent`, FileTree.svelte:43-60 — that is symbol structure, not the filesystem.)

**Workspace shape & resolution — `workspace.js` + `pds.js` + `+page.svelte`**
- `readWorkspace(root)` (workspace.js:30-51) walks the directory, finds the **shallowest `pds.toml`**
  and uses its directory as the workspace `base` (workspace.js:32-41), collects every `.pds` under
  `base`, and reads the `pds.toml` text. `root` (the `FileSystemDirectoryHandle`), `base`, and
  `manifestToml` are kept on the workspace object (workspace.js:50).
- **FQN derivation (load-bearing): `fqnOf(path, base)` (workspace.js:151-155).** The path is taken
  **relative to `base`**, the `.pds` extension dropped, and **`/` separators replaced with `::`** —
  `banking/core.pds` (base `""`) → `banking::core`. This mirrors LANG.md §8.1 (header comment,
  workspace.js:1-7). So renaming/moving a file must recompute its FQN with this exact rule, not by
  trimming the raw slash path.
- The page holds `workspace` (incl. `workspace.root` and `workspace.files`) and `moduleSources`
  keyed by FQN (+page.svelte:90-92). The whole model is derived from
  `allModules = files.map(f => ({ fqn: f.fqn, source: moduleSources[f.fqn] }))` (+page.svelte:137-139)
  and fed to `checkModules` (+page.svelte:143), `outlineModules`, `emitSceneModules`,
  `symbolScene`, `references`, etc. **So FQN — i.e. the path — is the resolution key everywhere.**
  There is no `compileWorkspace(files, entry)`; resolution is the reactive `$derived` chain off
  `allModules`. No explicit "entry file" concept is passed to wasm.
- `errorPaths` maps error-bearing module FQNs back to `f.path` (+page.svelte:159-168) to flag rows.

**FS handle availability for writes — `workspace.js`, `recents.js`**
- The root handle is opened `{ mode: "readwrite" }` up front (workspace.js:19), retained on
  `workspace.root`, and re-granted on reopen via `queryPermission`/`requestPermission`
  (recents.js:65-76). **A writable directory handle is available** — the foundation for all disk
  mutation is already present.
- Existing disk primitives in workspace.js: `readFile(handle)` (90-93), `writeFile(handle, text)`
  (createWritable+write+close, 96-100), `fileHandleAt(root, path)` (private — walks/creates dirs
  via `getDirectoryHandle({create:true})`, then `getFileHandle({create:true})`, 116-122), and
  `writeSite` (107-113). **Missing: any delete, rename, or move** — there is no `removeEntry` call
  anywhere. The FS Access API offers `dir.removeEntry(name, { recursive })` but has **no atomic
  rename/move**.

## Proposed approach (per-operation: memory + disk + resolution)

New helper module (`web-ide/src/lib/fs-ops.js`) owns disk mutation against `workspace.root`,
reusing `readFile`/`writeFile` from workspace.js and a parent-dir walk modelled on the private
`fileHandleAt` (116-122). `FileTree.svelte` grows action
affordances and dispatches new events; `+page.svelte` handles them: mutate disk, then update
`workspace.files` + `moduleSources` (keyed by the new FQN), which re-runs the reactive resolution
chain automatically. **Disk-first for delete; for rename/move, roll back the in-memory change if
the disk op throws.** No `compileWorkspace` call is needed — reassigning `workspace`/`moduleSources`
re-derives everything.

Helper signature sketch: resolve a path to its parent dir handle by walking `path.split("/")`
from `root` (the leaf's name is the last segment), then act.

### Rename (file)
- **Memory:** compute `newFqn = fqnOf(newPath, base)` (reuse the workspace.js rule, not a naive
  slice). Update the file's `{ path, fqn, handle }` in `workspace.files`; move
  `moduleSources[oldFqn] → [newFqn]`; if it's `openFile`, repoint `openFile` (and `openPath`).
- **Disk:** no rename primitive → resolve the parent dir handle, `getFileHandle(newName,
  {create:true})` → `writeFile` with the current source → `parentDir.removeEntry(oldName)`. Capture
  the fresh handle.
- **Resolution:** FQN changes ⇒ module identity changes. Any module importing the old FQN now
  dangles. **MVP: rename only; surface the resulting diagnostics. Do NOT auto-rewrite importers**
  (see open questions).

### Move (drag & drop between folders)
- **Memory:** identical to rename — only the directory prefix of `path` changes, but `fqn` must
  still be recomputed via `fqnOf` since the `::` prefix changes too.
- **Disk:** resolve/create the destination dir (walk like `fileHandleAt`), then create-at-new +
  write + `removeEntry` from the source dir. Since folders are implicit, no empty-dir cleanup is
  needed unless real folders are introduced (see new-folder).
- **Resolution / UI:** same FQN-change consequence as rename. UI = HTML5 drag-and-drop on file
  rows; drop target is a folder row (requires modelling folders — see below) or the root. Guard:
  no-op drop onto the same dir.

### Delete (file)
- **Memory:** drop the file from `workspace.files` and delete `moduleSources[fqn]`; if it was open,
  pick another file (or none).
- **Disk:** `parentDir.removeEntry(name)`.
- **Resolution:** recompute; a depended-on module's deletion surfaces dangling-import diagnostics
  (acceptable). Behind a confirm dialog.

### New folder + folder delete (overlaps T9/T10)
- **Folders are not modelled today.** To support folder targets for move, folder delete, and
  new-folder, the page must derive a directory tree from `files` paths (and track genuinely-empty
  dirs separately, since an empty dir has no file to imply it). Two options: (a) introduce real
  folder nodes in the tree model + an empty-dir set; (b) defer empty folders and treat "new folder"
  only as a path prefix chosen when creating a file. **Coordinate with T9/T10** — the folder model
  decision is shared and should not be made twice.
- Folder delete: `parentDir.removeEntry(name, { recursive: true })` + drop every file under the
  prefix from memory; recursive ⇒ confirm dialog.

## Affected / new files

- `web-ide/src/lib/components/FileTree.svelte` — add rename (inline edit), a per-row delete button,
  drag-and-drop handlers + drop highlight, folder rows + new-folder/folder-delete affordances;
  dispatch `rename`, `move`, `deletefile`, `deletefolder`, `newfolder`. **Conflicts with T9/T10**
  (same component).
- `web-ide/src/routes/+page.svelte` — handlers wiring those events to fs-ops; update
  `workspace.files` + `moduleSources` (rekey by FQN) + `openFile`/`openPath`; the existing reactive
  chain recomputes the model.
- `web-ide/src/lib/fs-ops.js` **(new)** — `renamePath`, `movePath`, `deletePath`, `deleteDir`,
  `makeDir` over a root `FileSystemDirectoryHandle`; reuses workspace.js's `readFile`/`writeFile`;
  unit-testable against a mock handle.
- `web-ide/src/lib/workspace.js` — export `fqnOf` and the dir-walk (currently the private
  `fileHandleAt`, 116-122) so fs-ops can reuse them; or add a small `parentDirFor(root, path)`.

## Open questions / decisions needed

1. **Filename couples to module name — confirmed.** `fqn = fqnOf(path, base)` — base-relative,
   `.pds` dropped, `/` → `::` (workspace.js:151-155) — and FQN is the model's key. So rename/move is
   NOT a pure FS op: it changes module identity and can break importers. **This is the load-bearing
   fact.** Decide: MVP surfaces diagnostics only, or
   auto-rewrites cross-module references? (Auto-rewrite needs a reference index — `references()`
   exists in pds.js:127 but is offset-based per-symbol, not a module-level "who imports FQN X".)
2. **Is there an entry-file concept to maintain?** The IDE does not pass an `entry` to wasm;
   resolution spans all modules equally. `pds.toml` may name docs/entry for the CLI (`docManifest`,
   pds.js:181) but the IDE doesn't key resolution off it. Confirm whether rename/move/delete must
   touch `pds.toml` at all (T8). If `pds.toml` references module paths, those need rewriting.
3. **Empty-folder modelling** — see new-folder; coordinate with T9/T10.
4. **Partial-failure semantics** for non-atomic rename/move (create+write+remove). Define rollback
   so a failed `removeEntry` after a successful write doesn't leave a duplicate.
5. **Permission lapse:** confirm the mutation path re-checks `requestPermission({mode:"readwrite"})`
   before first write in a session (open already requests it; a reopened handle re-grants in
   recents.js:65-76).

## Dependencies on other tasks

- **T3 (save):** reuse the existing `writeFile` createWritable path (workspace.js:76-80) — rename/
  move = write-new + remove-old.
- **T8 (pds.toml entry):** only if `pds.toml` references module paths; the IDE itself has no entry
  concept, so this dependency is conditional on #2 above.
- **T9 / T10 (create file/folder):** shared empty-folder model decision and the FileTree action-UI.
  **Direct merge-conflict surface in FileTree.svelte — sequence, do not parallelize.**
- **T13 (sync):** disk↔memory reconciliation; T11 writes must not fight an external-change watcher.

## Acceptance criteria (testable)

1. Renaming a non-imported `.pds` updates the on-disk filename, updates `workspace.files`
   (`path`/`fqn`/`handle`, FQN via `fqnOf`) and rekeys `moduleSources`, keeps the editor open on it,
   and the model recomputes with no spurious diagnostics.
2. Moving a file into another folder updates disk and memory; the file resolves under its new FQN;
   dropping onto its current folder is a no-op.
3. Deleting a file removes it on disk and from `workspace.files`/`moduleSources` behind a confirm;
   if it was open, the editor falls back to another file.
4. Deleting a folder removes it recursively on disk and drops all contained files from memory,
   behind a confirm dialog.
5. After any op, disk and the in-memory file set match (verifiable by re-reading `workspace.root`).
6. A failed disk write (permission denied / `removeEntry` throws) leaves both disk and memory in
   their pre-op state — no half-applied rename/move, no duplicate file.
7. `fs-ops.js` has unit tests against a mock `FileSystemDirectoryHandle` for rename, move, delete,
   delete-folder, including the rollback path.

## Rough size + parallel-safe?

**Size: L.** The gestures are M, but folder modelling (none today), FQN-rekeying across
`workspace.files`/`moduleSources`/`openFile`, the reference-fallout decision, and non-atomic
rename/move rollback push it to L.

**Parallel-safe: NO.** Heavy `FileTree.svelte` edits collide directly with **T9/T10** (create
file/folder, same component, shared folder model). Sequence with — or immediately after — T9/T10;
treat T8 as a conditional dependency.
