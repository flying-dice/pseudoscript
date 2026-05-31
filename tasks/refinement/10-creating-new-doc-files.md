# T10 — Creating new doc files (#web-ide)

## Summary
Add an in-IDE action to author a new markdown doc: create the `.md` file (conventionally under
the workspace `docs/` folder), register it in the `[[doc.sidebar]]` manifest in `pds.toml` so
the nav/site picks it up, seed it with a starter template, open it in the editor, and persist
it. The doc model is **manifest-driven, not path-convention-driven**: a doc page only appears
in the sidebar (and the built site) because `pds.toml`'s `[[doc.sidebar]]` lists it with a
`{ title, path }`. So "create a doc" is two coupled writes — a new `.md` file *and* a new
sidebar entry — distinct from T9 (a `.pds` module, which is auto-discovered by file walk with
no manifest edit). Folder workspaces can persist to disk via file handles; the bundled example
is in-memory/session-only.

NOTE: read against the working tree. Several `crates/pseudoscript-doc/src/*.rs` are 0-byte
placeholders mid-migration on this branch, but the web-ide files cited below are present and
real (`+page.svelte` is ~47 KB / 1315 lines). Line numbers are working-tree exact at time of
writing and may drift slightly.

## Current state (file:line) — doc discovery/nav conventions
- **A doc is a manifest-listed page, not a path** — there is no `isDocPath` / `docs/` regex
  anywhere. The Documentation sidebar comes entirely from `[[doc.sidebar]]` in `pds.toml`,
  parsed by `docManifest(toml)` (`web-ide/src/lib/pds.js:181`, shape
  `{ name?, theme?, logo?, sidebar: [{ title, items: [{ title, path }] }] }`,
  `pds.js:175`). The sample manifest
  (`web-ide/src/lib/samples/acme-tickets/pds.toml`) shows the format: a `[doc]` table
  (`name`, `theme`) plus repeated `[[doc.sidebar]]` groups, each with
  `items = [{ title = "...", path = "docs/overview.md" }, ...]`. `path` is relative to the
  workspace base (the dir holding `pds.toml`); the samples all put pages under `docs/`, but
  that prefix is a convention in the manifest, not enforced in code.
- **Nav ordering** — explicit, manifest order. The sidebar renders groups and items in the
  order they appear in `[[doc.sidebar]]` (no alphabetical sort). So a new doc's position is
  wherever its entry is inserted in the manifest — the create flow decides placement.
- **Doc title** — the *manifest* `item.title` is the nav label (FileTree shows `{item.title}`,
  `FileTree.svelte:85`); the markdown body's `# H1` is independent. Sample docs each open with
  a single `# H1` (e.g. `web-ide/src/lib/samples/acme-tickets/docs/overview.md:1`
  `# ACME Tickets`) and have **no YAML front-matter**. So a new doc needs a `title` in the
  manifest entry *and* (by convention) a leading H1 in the body.
- **Load path** — on mount, `loadWorkspaceDocs(ws)` (`+page.svelte:473`) parses
  `ws.manifestToml` via `docManifest`, then loads each page's content: for a folder via
  `readDocPages(ws.root, ws.base, manifest.sidebar)` (`workspace.js:61`, which opens each
  `base/path` through a handle, dropping pages that don't exist — warn-and-skip), or for a
  bundled sample via `sampleDocPages(manifest.sidebar, ws.docs)` (`+page.svelte:672`). Results
  populate `docGroups` (sidebar + per-item `handle`), `docSources` (path → live Markdown), and
  `docMeta` (`+page.svelte:489-494`).
- **FileTree (the create surface, today read-only)** — `FileTree.svelte` props are
  `{ workspaceName, files, openPath, onopen, onpicknode, errorPaths, docGroups, ondocopen,
  symbols, selectedFqn }` (`FileTree.svelte:14-31`). The Documentation section
  (`FileTree.svelte:69-92`) lists `docGroups[].items[]` as buttons calling `ondocopen?.(item)`.
  There is **no** create/rename/delete callback and **no** `oncreate`/context menu — confirmed
  by grep (no `createDoc`/`newDoc`/`newFile`/`addFile`/`oncreate` anywhere in `web-ide/src`).
  T10 is greenfield UI wiring.
- **Opening a doc** — `openDoc(item)` (`+page.svelte:587`) sets
  `openFile = { isDoc: true, path, title, handle }`. The editor then renders Markdown
  (`Editor` gets `markdown={openFile.isDoc}`, `+page.svelte:899`) using the live-preview
  bundle `markdownLivePreview()` (`web-ide/src/lib/markdown-live.js:520`). Source resolves from
  `docSources[openFile.path]` (`+page.svelte:128-129`). So once a doc is registered + loaded +
  opened, editing/preview works with no extra work.
- **Persistence** — two mechanisms:
  - *Folder workspace* (`ws.root` set): per-page edits autosave through the page's file handle
    — `onEditorChange` routes doc edits to `docSources` then `scheduleSave(openFile.handle, …)`
    → `writeFile(handle, text)` (`+page.svelte:565-573`, `workspace.js:96`). A **new** doc has
    no handle yet; the create flow must create the file on disk. `workspace.js` has
    `fileHandleAt(root, path)` (`:116`) which walks/creates dirs and `getFileHandle(name,
    { create: true })` — exactly the primitive to create `docs/foo.md` (and auto-create
    `docs/`). It is currently private (only used by `writeSite`); T10 would export/reuse it.
    The `pds.toml` itself is read into `ws.manifestToml` but the IDE has **no** code today that
    *writes* `pds.toml` back — adding a sidebar entry requires writing the manifest file, a new
    capability (overlaps T8 "editing pds.toml").
  - *Bundled sample* (`ws.root` null): everything is in memory; `scheduleSave` no-ops when
    `handle` is null (`+page.svelte:560`). A new doc would live only in `docSources` +
    `docGroups` for the session.
- **Site build** — `runBuild()` (`+page.svelte:683`) calls `renderDocSite(allModules,
  buildDocConfig())`; `buildDocConfig` (`+page.svelte:659`) emits the sidebar from `docGroups`
  with each item's live `docSources` content. So a doc that's been added to `docGroups` +
  `docSources` is included in the built site even before the manifest is persisted — but it
  won't survive reload unless `pds.toml` is written.
- **Doc crate** — `crates/pseudoscript-doc` (the `pds doc` static-site generator) is
  mid-migration on this branch (several `src/*.rs` are 0-byte). The committed `docManifest`/
  `renderDocSite` wasm API is what the IDE uses; its manifest contract (`[[doc.sidebar]]`,
  `path` relative to base, `title` as label, no front-matter) is the convention T10 must honor.

## Proposed approach
1. **Entry point** — add a "New doc" affordance to the Documentation section of
   `FileTree.svelte` (a "+" by the section head and/or per-group), emitting a new
   `oncreatedoc` callback. Wire it in `+page.svelte`.
2. **Prompt** — ask for a display **title** (the manifest `item.title`) and derive a filename
   slug (lowercase-kebab) under `docs/`, ensuring a `.md` extension; or prompt title + path
   separately. Reject empty; reject a `path` already present in the manifest.
3. **Starter template** — seed the body with a leading H1 from the title plus a one-line
   placeholder (matches the sample convention; no front-matter):
   ```
   # <Title>

   <one-line description>
   ```
4. **Register + create + persist** in a `createDoc(title, path, group?)` handler:
   - Add `{ title, path, handle }` to the chosen `docGroups` group (or a default group),
     and set `docSources[path] = template`, so the sidebar + preview update immediately
     (reactive).
   - Append the entry to `[[doc.sidebar]]` in `pds.toml` and **write `pds.toml` back** (folder
     mode) so it survives reload. This is the new persistence capability; coordinate with T8.
   - Folder mode: create the `.md` on disk via `fileHandleAt(ws.root, `${ws.base}/${path}`)` +
     `writeFile`, and store the resulting handle on the item so subsequent edits autosave.
   - Sample/in-memory mode: keep it session-only (or gate per T4).
5. **Open it** — call `openDoc(item)` so it loads in the Markdown editor and the doc-width
   toggle/preview engage.

## Affected/new files
- `web-ide/src/lib/components/FileTree.svelte` — add a "New doc" control in the Documentation
  section and an `oncreatedoc` prop/callback. (Currently has no create UI.)
- `web-ide/src/routes/+page.svelte` — add `createDoc()`, the prompt, the template, mutation of
  `docGroups`/`docSources`, the disk write (`.md` + manifest), and `openDoc` of the new page.
- `web-ide/src/lib/workspace.js` — export/reuse `fileHandleAt` for creating the `.md`; add a
  `writeFile`-based helper to persist `pds.toml` (new — manifest serialization), or a focused
  "append sidebar item" helper. Possibly a `serializeManifest`/`addSidebarItem` util.
- (Maybe) `web-ide/src/lib/pds.js` — if manifest round-tripping is done in wasm rather than JS,
  a serialize counterpart to `docManifest`. Decide JS-side vs wasm-side.
- No changes needed to `markdown-live.js` (preview already works for any doc).

## Open questions / decisions needed
- **Manifest write is the crux.** The IDE currently never writes `pds.toml`. Adding a sidebar
  entry means serializing TOML (preserving the rest of the file, comments, formatting) or doing
  a surgical append. Decide: JS TOML writer vs wasm helper vs naive text-append of a
  `[[doc.sidebar]]`/item. Strongly overlaps **T8 (editing pds.toml)** — likely a shared
  dependency.
- **Which sidebar group?** Append to an existing group (which one?), prompt for group, or
  create a new group. Recommend: prompt-select existing group or "new group".
- **Title vs path prompt** — prompt title and derive `docs/<slug>.md`, or prompt both. Recommend
  title-only with derived path for v1.
- **`docs/` placement** — convention only; allow nesting (`docs/guides/x.md`)? `fileHandleAt`
  supports it.
- **In-memory mode** — is creating docs offered for the bundled sample (session-only), or
  folder-only? Hinges on T4.
- **Collision** — reject (recommended) vs auto-suffix; also handle a path that exists on disk
  but isn't in the manifest.
- **Front-matter** — none today; keep template H1-only. If the doc crate later adopts
  front-matter for title/order, template + `docManifest` evolve together (follow-up).

## Dependencies on other tasks (T4 folder-only docs, T9 new files, T11 FS, T12 md formatting)
- **T8 (editing pds.toml)** — *not in the listed set but the strongest real dependency*:
  registering a doc requires writing `[[doc.sidebar]]` into `pds.toml`, which T8 owns. T10
  should build on T8's manifest-write capability rather than inventing a second one.
- **T4 (folder-only docs + link/image resolution)** — strong. T4 gates doc preview to
  folder-backed workspaces. If docs are folder-only, T10 targets folder mode (disk `.md` +
  manifest write) and can skip/disable the in-memory path. Coordinate on whether create is
  offered for the bundled sample.
- **T9 (new .pds files)** — *less overlap than it appears*. T9 creates a `.pds` module, which
  is auto-discovered by the workspace file walk (`readWorkspace`, `workspace.js:30`) with **no
  manifest edit**. T10 additionally requires a `[[doc.sidebar]]` entry. They can share a
  filename-prompt + collision + disk-create primitive, but T10's manifest-registration step is
  unique. Co-design the shared create primitive; keep doc-registration separate.
- **T11 (filesystem)** — owns the FS layer in `workspace.js`
  (`fileHandleAt`/`writeFile`/`readFile`/handles). T10 consumes it to create the `.md` and to
  write `pds.toml`; if T11 reworks handle/dir-creation/permissions, T10 rides on that API.
- **T12 (md formatting)** — adjacent, not blocking. T12 affects doc body formatting/rendering;
  T10 only needs the seed template to render cleanly under `markdownLivePreview`. Keep the
  template consistent with T12.

## Acceptance criteria (testable)
- A "New doc" control exists in the FileTree Documentation section.
- Creating a doc titled "Release Notes" produces a `docs/release-notes.md` (or chosen path) and
  a matching `[[doc.sidebar]]` item `{ title = "Release Notes", path = "docs/release-notes.md" }`.
- The new page appears in the Documentation sidebar at its inserted position, labelled by its
  manifest `title`.
- The body is seeded with a leading `# Release Notes` (not empty).
- Selecting it opens it in the Markdown editor (`openDoc`), source from `docSources`, rendered
  via `markdownLivePreview`.
- Folder mode: the `.md` is created on disk under the workspace base (with `docs/` auto-created
  if absent), `pds.toml` is updated and written, and both survive reopening the folder.
- The new doc is included when "Build docs" runs (present in `buildDocConfig`/`renderDocSite`).
- A path that collides with an existing manifest entry is rejected with a visible hint and no
  mutation.
- Editing the new doc autosaves to its handle (folder mode) like an existing page.

## Rough size (S/M/L) + parallel-safe?
**M.** The preview/open/edit/build path already exists end-to-end; the new work is real but
bounded: a FileTree control, a create handler, a template, disk creation of the `.md`, and —
the hard part — writing a `[[doc.sidebar]]` entry into `pds.toml` (manifest serialization the
IDE doesn't do yet). The manifest write pushes it above S and creates a hard coupling to **T8**.
**Parallel-safe with caution** — edits `+page.svelte`, `FileTree.svelte`, and `workspace.js`,
all touched by T8/T9/T11; sequence after T8 (manifest write) and T4 (folder-only gating).
