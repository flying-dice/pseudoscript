# T7 — Import/export compressed workspace file (#web-ide)

## Summary
Export the whole in-memory IDE workspace (`pds.toml` + every `.pds` module + `docs/*.md`) to one downloadable compressed file (`.pdsz`), and import that file back to reconstruct the workspace in the IDE. This is the sibling of T6 (compressed share URL): both move the *same* workspace snapshot across a boundary (download/upload vs URL), so they MUST share one canonical serialization. Recommendation: a single `web-ide/src/lib/codec.js` producing a versioned JSON manifest, gzipped via the platform `CompressionStream`; T7 wraps those bytes in a `.pdsz` download, T6 base64url-encodes the same bytes into a URL.

## Current state (file:line) — workspace model + any serialization

The `web-ide` SvelteKit app exists and is the host. The working tree is ahead of the last web-ide commit (1287620): `samples.js`, `recents.js`, `ProjectPanel.svelte`, `Settings.svelte`, `Notifications.svelte` and the `samples/` catalogue are present but uncommitted. This refinement targets the working-tree state.

**In-memory workspace shape** — `web-ide/src/routes/+page.svelte`:
- `workspace = $state(null)` (`:90`) holds `{ name, root?, base?, sampleId?, manifestToml, files, docs? }`. `root` is a `FileSystemDirectoryHandle` for a folder workspace, `null`/absent for a sample.
- `workspace.files` is the file map; each entry is `{ path, fqn, source?, handle? }` — `source` (in-memory text) for samples, `handle` (a `FileSystemFileHandle`) for folder workspaces. `fqn` is derived from the path. The file map carries **only `.pds` modules**, not `pds.toml`/docs (those are separate, below).
- `manifestToml` — the raw `pds.toml` text, kept for the doc build.
- `docGroups`/`docSources`/`docMeta` (`:96-98`) — authored `[[doc.sidebar]]` pages: `docSources` is path→Markdown text.
- `mountWorkspace(ws, landing)` (`:453`) is the **single entry point** that swaps in a freshly-loaded workspace and resets navigation — import reuses it. It expects `moduleSources` to be seeded first: `openSample` (`:499-506`) does `moduleSources = Object.fromEntries(loaded.workspace.files.map((f) => [f.fqn, f.source]))` then `mountWorkspace(...)`; `openRecent` (`:510-532`) seeds from disk via `readFile(file.handle)` then mounts. So import must build the same `{ name, files:[{path,fqn,source}], manifestToml, docs }` object, seed `moduleSources` from it, then call `mountWorkspace`. Doc pages load via `loadWorkspaceDocs(ws)` (`:473-495`), which for a handle-free workspace reads `ws.docs` (the `{ relPath: markdown }` map).
- `saveActive`/disk persistence: edits to folder files debounce-write through `writeFile(handle, text)` (`:562`); sample edits live only in `moduleSources` for the session.

**No existing share serialization (gap vs the task brief).** `web-ide/src/routes/` has only `+layout.js` and `+page.svelte` — there is no `routes/api/share`, no `routes/s/[id]`, and no compress/gzip/base64/JSZip code anywhere in `src` (grep confirms zero hits). T6's URL share is also greenfield. So T6+T7 jointly *introduce* the canonical format; there is no prior format to align to.

**Workspace producers (the shapes import/export must match):**
- `web-ide/src/lib/samples.js:78-94` — `loadSample(id)` returns `{ workspace: { name, root:null, sampleId, files:[{ path, fqn, source, handle:null }], manifestToml, docs }, landing }`. `docs` is a `{ relPath: markdown }` map. The catalogue is built from `import.meta.glob("./samples/*/*.pds" | "*/pds.toml" | "*/**/*.md", { query:"?raw", eager:true })` (`:13-31`). This in-memory, handle-free shape is the closest analogue to import — **build the imported workspace exactly like this** so `mountWorkspace` consumes it unchanged.
- `web-ide/src/lib/workspace.js:30-51` — `readWorkspace(root)` (FS Access) returns `{ name, root, base, manifestToml, files:[{ path, fqn, handle }] }`. `fqnOf` (`:151-155`) derives the FQN from the path relative to `base` (`LANG.md` §8.1: separators → `::`). `readFile(handle)` (`:90-93`) reads live text; `writeFile`/`fileHandleAt` (`:95-122`) are the disk-write primitives. For a folder workspace, **export must read current text via `readFile(handle)`**, not a stale buffer.
- `web-ide/src/lib/recents.js` — `getRecents()` returns `{ key, kind, name, sampleId?, at }[]` from `localStorage` (`:32-35`); `recordSample` / `recordFolder` (`:43-59`) upsert entries (folders also persist their handle in IndexedDB via a raw `indexedDB` wrapper — `idb` is **not** a dependency). `forget(key)` removes one. An imported workspace needs a new recorder, e.g. `recordImported(name)` writing `{ key: "imported:<name>", kind:"imported", name, at }` — but a re-openable import has no handle and no bundled source, so see open question 4.

**Toolbar** — `web-ide/src/lib/components/Toolbar.svelte:1-49` is a pure presentational component: its props are callbacks (`onformat`, `onproject`, `onbuilddocs`, `onopensettings`) wired from `+page.svelte`. Adding Import/Export = two new buttons + two new callback props here, with handlers in `+page.svelte`.

**Available deps / platform** — `package.json` has no zip/gzip/compression library. The app already requires Chromium (File System Access, `workspace.js:9-11`), where `CompressionStream`/`DecompressionStream` are available — so gzip needs **no new dependency**.

## Proposed approach (format, shared codec with T6, import/export flows)

### Canonical workspace manifest (shared with T6)
One JSON object is the single source of truth for both tasks:
```jsonc
{
  "format": "pdsz",
  "v": 1,
  "name": "ticketing",          // workspace name
  "files": {                     // workspace-root-relative path -> UTF-8 text
    "pds.toml": "...",
    "internet_banking.pds": "...",
    "docs/overview.md": "..."
  }
}
```
- `files` is a flat path→text map covering **`pds.toml` + every `.pds` + every authored `docs/*.md`** (note: the live model splits these into `manifestToml` / `files` / `docSources`; the manifest reunites them into one map, and deserialize splits them back out).
- Text-only workspace (`.pds`/`.toml`/`.md`), so no binary handling — skip/reject non-UTF-8.
- On import, each `.pds` file's `fqn` is re-derived from its path the same way `fqnOf` (`workspace.js:152`) / the sample loader do, so the manifest need not store `fqn`.
- `format` + `v` are the version gate: importers MUST reject an unknown `format` and MUST refuse a `v` higher than they support.

### Container format decision: gzipped JSON, not zip
- Payload is a handful of small text files. `gzip(JSON.stringify(manifest))` via `CompressionStream` is a few lines, zero new deps, and yields bytes T6 can base64url straight into a URL. JSZip would add a heavy dependency and a second, divergent encode path for T6.
- Download extension `.pdsz` (gzip bytes). Feature-detect `CompressionStream`; fall back to uncompressed `.json` only if absent (it is present on the supported Chromium target).
- If raw-zip interop with external tools is ever needed, add it later as a second exporter without touching the manifest.

### Shared codec module — `web-ide/src/lib/codec.js` (new)
Both T6 and T7 import this; the format never forks.
- `serializeWorkspace({ name, manifestToml, files, docSources }) -> Promise<manifest>` — flatten the live state into the manifest's `files` map: `pds.toml` ← `manifestToml`, each `.pds` ← its current text (`f.source ?? readFile(f.handle)`), each doc path ← `docSources[path]`. Async because folder files read from disk.
- `deserializeWorkspace(manifest) -> ws` — validate `format`/`v`, then split the flat map back into the `mountWorkspace` shape: `pds.toml` → `manifestToml`; `*.pds` → `files:[{ path, fqn, source, handle:null }]` (FQN re-derived); `docs/*` → a `docs` map (the sample shape). Result is handle-free/in-memory, exactly like `loadSample`.
- `encode(manifest) -> Promise<Uint8Array>` — JSON → `TextEncoder` → gzip (`CompressionStream`).
- `decode(bytes) -> Promise<manifest>` — gunzip → `TextDecoder` → `JSON.parse` → schema-validate.
- `encodeForUrl(manifest)` / `decodeFromUrl(str)` — base64url wrappers over encode/decode, owned here for T6.

### Export flow (T7)
Toolbar "Export" → `+page.svelte` handler → `serializeWorkspace(...)` over the current `workspace`/`docSources` → `encode()` → `new Blob([bytes])` → anchor-download named `<slug(workspace.name)>.pdsz` (fallback `workspace.pdsz`). Pure client-side; for a folder workspace it reads live disk text first via `readFile`.

### Import flow (T7)
Toolbar "Import" → hidden `<input type="file" accept=".pdsz,.json">` (and/or drag-drop onto the editor) → read `ArrayBuffer` → `decode()` → `deserializeWorkspace()` → seed `moduleSources` from `ws.files` (as `openSample` does, `:502`) → `mountWorkspace(ws, landing)` (`+page.svelte:453`) → record in recents (open question 4) → `refreshRecents()`.
- **Conflict handling:** a workspace is always loaded once the user picks one, so import replaces the current workspace. Use the existing `Notifications` surface / a confirm to gate: *Replace* (load imported) vs *Cancel* (no change). Dirty-buffer guard: warn if there are unsaved sample edits before replacing. (Folder workspaces persist to disk on save, so their on-disk state is safe regardless.)
- **Validation failures** (bad gzip, bad JSON, unknown `format`, future `v`, empty `files`) surface an error notification (`flash(...)`, used at `:562`) and leave the current workspace untouched.

## Affected/new files
- **New** `web-ide/src/lib/codec.js` — canonical serialize/deserialize + gzip/base64url codec (shared with T6).
- **New** test for `codec.js` — round-trip + validation. (No JS test runner is configured in `web-ide` yet; T7 either adds Vitest or the test rides whatever T6 introduces — flag in DoD.)
- `web-ide/src/lib/components/Toolbar.svelte` — add Import + Export buttons and `onimport`/`onexport` callback props (mirrors the existing `onformat`/`onbuilddocs` pattern, `:5-10`).
- `web-ide/src/routes/+page.svelte` — wire handlers, file-input element, optional drag-drop target, conflict confirm; seed `moduleSources` and call `mountWorkspace` plus the recents recorder. Export reads live text via `readFile` for handle-backed files.
- `web-ide/src/lib/recents.js` — add a `recordImported(name)` recorder (and let the project panel offer it), or decide imports are not re-openable (open question 4).
- (T6) `web-ide/src/routes/api/share/` + `s/[id]/` if/when T6 adds them — they MUST call `codec.js`.

## Open questions / decisions needed
1. **zip vs gzip-JSON** — confirm gzipped-JSON manifest over JSZip (recommended for code size + T6 reuse).
2. **`pds.toml` `[deps]` on import** — load the `pds.toml` text verbatim; defer git/path dependency resolution to the existing `pds add/install` path. Confirm.
3. **Format ownership** — agree the manifest schema lives in `codec.js`, co-owned with T6; any field change is version-bumped.
4. **Are imports re-openable from recents?** A recents entry needs a way to reload. Folders store a handle; samples store an id. An import has neither unless we persist its bytes (e.g. in IndexedDB). Options: (a) record it but require a fresh file pick to reopen, (b) persist the `.pdsz` bytes in IndexedDB for true reopen, (c) don't add to recents at all. Recommend (a) for the first cut.
5. **Conflict UX** — Replace/Cancel + dirty-buffer guard sufficient, or also "open as new"? No multi-workspace concept exists, so Replace is the natural default.
6. **Browser floor** — `CompressionStream` is fine given the app already needs Chromium (FS Access, `workspace.js:9`). Confirm no pako fallback required.
7. **Does export include generated `target/doc` output?** Recommend no — export source only (`pds.toml` + `.pds` + authored `docs/*.md`); exclude `target/`, `node_modules`, dotdirs (matches `workspace.js:13` `SKIP_DIRS`).

## Dependencies on other tasks
- **T6 (compressed share URL)** — HARD shared-format dependency. Both consume `codec.js`; the manifest schema and version gate are co-designed. Build the codec once (in whichever lands first); the other imports it. No prior share format exists, so this is a clean joint introduction.
- **T3 (workspace init/save)** — provides folder save/persist. Independent for *import display* (imported workspaces live in-memory and reuse the sample/`mountWorkspace` path), but "save an imported workspace to a new folder" needs T3's write-to-folder flow (the primitives `writeFile`/`fileHandleAt`/`writeSite` already exist in `workspace.js`).

## Acceptance criteria (testable)
1. Export produces a `.pdsz` whose decoded manifest `files` map contains the `pds.toml`, every `.pds`, and every authored `docs/*.md` of the open workspace, byte-identical text.
2. Round-trip: `deserializeWorkspace(decode(encode(await serializeWorkspace(ws))))` yields a workspace whose files (paths + text) and `manifestToml` deep-equal the original; `.pds` FQNs re-derive identically.
3. `codec.js` is the only place the manifest is (de)serialized; T6's URL path calls it — a test asserts the URL and file paths produce the same manifest for the same workspace.
4. Importing into the loaded workspace shows Replace/Cancel; Cancel leaves `workspace.files` and the open file unchanged.
5. A successful import surfaces in recents per the open-question-4 decision (or, if (c), explicitly does not).
6. Corrupt input (truncated gzip, invalid JSON, unknown `format`, `v` too high, empty `files`) shows an error and does not mutate the current workspace — each covered by a test.
7. Export filename derives from `workspace.name` (slugified), `workspace.pdsz` fallback.
8. No new runtime dependency (gzip via `CompressionStream`).

## Rough size + parallel-safe?
- **Size: M.** Codec + flows are small; the real surface is the shared-format coordination with T6, the manifest↔state split (manifest reunites `manifestToml`/`files`/`docSources` that the model keeps separate), and the conflict/dirty-buffer UX. No host-app blocker — the IDE and its workspace model already exist.
- **Parallel-safe: partially.** Best designed alongside T6 (shared codec — one author or tight coordination). The import path is independent of T3; saving an imported workspace to a folder depends on T3. Clean split: one task authors `codec.js`, T6 and T7 build on it.
