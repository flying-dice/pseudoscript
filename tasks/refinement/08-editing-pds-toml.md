# T8 — Editing pds.toml (#web-ide)

## Summary (+ raw vs form decision)

Today the web IDE loads `pds.toml` once on open, reads it for the doc build, and
never lets the user edit it: it is not in the file tree, has no editor handle,
and is never written back. This task adds manifest editing.

**Decision: ship a raw TOML editor first, layer a structured form on top
later.** The manifest is small, hand-authored, and its real complexity is the
`[[doc.sidebar]]` array-of-tables and the `[dependencies]` table — both awkward
in a naive form but trivial as text. The IDE already owns a full CodeMirror 6
editor (`Editor.svelte`) and a wasm TOML parser (`docManifest` →
`wasm_doc_manifest`), so the raw path reuses existing machinery and is low-risk.
A structured form (name, theme dropdown, logo, sidebar group/item rows, deps
rows) is the better UX but is strictly additive and should be a follow-up once
the read/parse/write/re-resolve loop is proven. Recommend building raw now,
form as a fast-follow that round-trips through the same write+re-resolve path.

The load-bearing realisation: there is **no `[workspace] name/entry` schema** as
the ticket framing assumed. The manifest is a `[doc]` table plus `[dependencies]`,
and the module set (the "entry") is *derived by walking `.pds` files*, not named
in the manifest. So "changing entry re-resolves the model" does not apply as
written; what re-resolves the model is editing `.pds` files (already handled) or
editing `[dependencies]` (needs `pds install`, out of browser scope — see Open
questions). Editing `[doc].*` and `[[doc.sidebar]]` only affects the **doc-site
build**, not the model/diagrams. This reframing matters for acceptance criteria.

## Current state (file:line) — schema + current handling

**Authoritative schema (CLI is source of truth):**

- `crates/pseudoscript/src/workspace.rs:43-71` — the `[doc]` table:
  `name` (string, default = root dir name), `out` (string, default
  `target/doc`), `logo` (string, optional), `theme` (`light`|`dark`, default
  `dark`), and repeated `[[doc.sidebar]]` groups. `workspace.rs:60-71` —
  each sidebar group is `{ title, items: [{ title, path }] }`; `title` defaults
  to `path` when blank. Theme validation: `workspace.rs` `parse_theme`
  rejects anything but `light`/`dark`.
- `crates/pseudoscript/src/deps.rs:38-54` — `[dependencies]` map; each entry is
  `DepSpec { git: String, rev: Option<String>, path: Option<String>,
  name: Option<String> }`. Managed by `pds add` / `pds install`, not by hand.
- There is **no `entry` field and no `[workspace]` table.** Module FQNs derive
  from each `.pds` file's path relative to the manifest dir (`workspace.rs`
  `module_fqn`, mirrored in JS at `web-ide/src/lib/workspace.js:151-155`).

**How the IDE handles pds.toml today (read-only):**

- `web-ide/src/lib/workspace.js:30-51` — `readWorkspace` walks the picked
  folder, finds the shallowest `pds.toml`, and returns its raw text as
  `manifestToml` (line 48) plus `base` (the manifest dir). The walk filter at
  `workspace.js:42-43` keeps only `*.pds` — **`pds.toml` itself is never added
  to `files`**, so it cannot be opened in the tree.
- `web-ide/src/routes/+page.svelte:30-31` — `manifestToml` is destructured and
  stored on `workspace`. It is **never reassigned anywhere** and **no
  `writeFile` ever targets `pds.toml`** → manifest is immutable in-session.
- `web-ide/src/routes/+page.svelte:207-210` — the doc build is the only consumer:
  `docManifest(workspace.manifestToml ?? "")` → `readDocPages(root, base,
  manifest.sidebar)` → `config = { name, theme, logo, docs }` →
  `renderDocSite(modules, config)`.
- `web-ide/src/lib/pds.js:181-183` — `docManifest(toml)` wraps
  `wasm_doc_manifest`, the **same TOML parser the CLI uses**; returns
  `{ name?, theme?, logo?, sidebar: [{title, items:[{title,path}]}] }`, empty
  `{ sidebar: [] }` for no `[doc]` table, throws only on malformed TOML. This is
  the validation primitive to reuse.
- `web-ide/src/lib/workspace.js:96-100` — `writeFile(handle, text)` already
  exists for persisting `.pds`/doc-page edits; the manifest write reuses it.
- `web-ide/src/lib/components/ProjectPanel.svelte:14-25` — workspace sidebar;
  props `workspace, activePath, onSelect, onOpenFolder, onOpenSample,
  onNewModule, onBuildDoc, onCloseFolder, busy`. Natural host for an
  "Edit manifest" affordance.
- `web-ide/src/lib/components/Settings.svelte:14-17` — **not** a manifest editor:
  it is the keyboard-shortcuts modal (`open`/`onClose`, `keybindings`). Do not
  overload it for the manifest.
- Editor library: **CodeMirror 6** (`@codemirror/*` in `package.json`), driven
  by `Editor.svelte`.

## Proposed approach

1. **Surface pds.toml as an openable buffer.** Carry the manifest's file handle
   out of `readWorkspace` (it already grabs `manifestHandle` internally at
   `workspace.js:33,37`; currently discards it after reading text at line 48).
   Return `{ manifestToml, manifestHandle, ... }`. Add a dedicated "manifest"
   pseudo-entry in `ProjectPanel` (above the module tree, not inside `files`,
   to avoid it being treated as a `.pds` module) that opens the manifest text in
   the existing CodeMirror editor with a TOML language mode (add
   `@codemirror/lang-... `/a minimal TOML mode, or plain text + diagnostics if a
   TOML grammar is not worth pulling in).
2. **Edit + persist.** On edit, `writeFile(manifestHandle, text)`
   (`workspace.js:96`), and update `workspace.manifestToml` in `+page.svelte`
   state (the field that is currently write-once at line 31).
3. **Re-resolve on save.** Distinguish two effects:
   - `[doc]`/`[[doc.sidebar]]` changes → re-run the doc build path
     (`+page.svelte:207-210`): re-`docManifest`, re-`readDocPages`, rebuild
     `config`, re-`renderDocSite`. Wire through the existing `onBuildDoc` flow so
     a manifest save invalidates/rebuilds the site.
   - `[dependencies]` changes → cannot be resolved in-browser (needs git fetch +
     `pds install`, `deps.rs`). Surface as an informational diagnostic ("save
     and run `pds install` in the CLI"); do not attempt to fetch.
4. **Structured form (follow-up).** A toggle ("Form ⇄ TOML") rendering name
   input, theme `<select>` (light/dark), logo path, repeatable sidebar
   group/item rows, and read-only deps rows. The form serialises back to TOML
   through one writer and flows through the exact same save+re-resolve path, so
   no second source of truth.

## Affected/new files

- `web-ide/src/lib/workspace.js` — return `manifestHandle` from
  `readWorkspace`/`openWorkspace` (lines 30-51); reuse `writeFile` for the
  manifest.
- `web-ide/src/routes/+page.svelte` — store `manifestHandle`; add open-manifest
  + save-manifest handlers; on save, re-run the doc-build block (207-210) and
  refresh state.
- `web-ide/src/lib/components/ProjectPanel.svelte` — "Edit manifest" entry +
  `onOpenManifest` prop.
- `web-ide/src/lib/components/Editor.svelte` — accept a TOML language mode (or a
  per-buffer language switch keyed off the open file's extension).
- **New:** `web-ide/src/lib/manifest.js` — validate (via `docManifest`) and, for
  the form, serialise the structured shape back to TOML; single owner of the
  manifest read/validate/write contract.
- **New (follow-up):** `web-ide/src/lib/components/ManifestForm.svelte` — the
  structured editor.
- `package.json` — possibly a CodeMirror TOML language package.

## Validation strategy

- **TOML parse errors:** run `docManifest(text)` (`pds.js:181`) on edit; it
  throws on malformed TOML — catch and show inline as an editor diagnostic /
  problem-pane entry (reuse the existing `ProblemsPane` channel). Same parser as
  the CLI, so parity is free.
- **Unknown sidebar page path:** for each `[[doc.sidebar]]` `item.path`, attempt
  `openFileAt(root, base+path)` (`workspace.js:76`) — a miss is a warning, not
  an error, matching the CLI's warn-and-skip (`workspace.js:66-68`,
  `workspace.rs` `load_doc_group`).
- **Bad theme:** validate `theme ∈ {light, dark}` in the form/serialiser,
  mirroring `parse_theme` (`workspace.rs`).
- **Missing logo:** warn-and-skip, like the CLI (non-fatal).
- **Bad dep refs:** the IDE cannot resolve git deps; validate only shape
  (`git` present per `DepSpec`, `deps.rs:40-53`) and prompt to run `pds install`.

## Open questions / decisions needed

- `[dependencies]` editing in-browser: confirm it is out of scope (no git/install
  in wasm). Recommend read-only display + "run `pds install`" prompt.
- Should `out` be editable from the IDE? The browser writes the site via
  `writeSite(root, files, dir)` (`workspace.js:107`); honoring a custom `out`
  means threading `manifest`-derived `out` into that call (currently hardcoded
  default `target/doc`).
- TOML language mode: pull a CodeMirror TOML grammar (extra dep) vs plain-text
  buffer + parse-on-save diagnostics. Lean plain-text for v1.
- Live re-render cadence: re-build doc site on every keystroke (debounced) vs
  on explicit save/build. Recommend on save to match the existing `onBuildDoc`
  model.
- Where does the manifest buffer live in the tree UI — a fixed top entry vs a
  toolbar button? (UX call for the design stage.)

## Dependencies on other tasks

- **T1/T2 (editor/workspace core):** depends on the existing CodeMirror
  `Editor.svelte` and the `readWorkspace`/`writeFile` plumbing being stable;
  this task extends, not replaces, them.
- **T9 (new files for entry):** related — both touch how non-`.pds` artefacts are
  created/opened. There is no manifest "entry" field, but new-`.pds`-file
  creation (T9) and manifest editing share the file-creation/open affordances in
  `ProjectPanel`; coordinate the sidebar UI.
- **T11 (FS management):** depends on it for the manifest handle lifecycle
  (persisting/regranting handles across sessions, like recents). Manifest writes
  go through the same FS-Access write path T11 governs.

## Acceptance criteria (testable)

1. With a folder workspace open, the user can open `pds.toml` in the editor
   (it is reachable from `ProjectPanel`, not silently dropped as today).
2. Editing the manifest and saving persists the new text to the on-disk
   `pds.toml` via the file handle (verifiable by reopening the folder).
3. Malformed TOML on save produces a visible diagnostic and does **not** corrupt
   the build (the previous valid manifest stays in effect or the build is
   blocked with a clear message).
4. Editing `[doc].name`/`theme`/`logo` and rebuilding the doc site reflects the
   change in the generated site (name in header, theme applied).
5. Adding/removing a `[[doc.sidebar]]` group/item and rebuilding changes the
   site's sidebar accordingly; an item pointing at a missing file is skipped with
   a warning, not a hard failure (parity with CLI).
6. A `[dependencies]` edit shows the "run `pds install`" guidance and does not
   silently appear to take effect.
7. (Form follow-up) Toggling Form ⇄ TOML round-trips: form edits serialise to the
   same TOML a hand-edit would, and re-parse identically.

## Rough size + parallel-safe?

**M** for the raw-edit path (open + write + re-resolve + validation), **+S/M**
for the structured form follow-up.

**Parallel-safe: mostly.** It touches `+page.svelte`, `ProjectPanel.svelte`,
`workspace.js`, and `Editor.svelte` — files also in scope for T9 (new files) and
T11 (FS mgmt). Sequence after, or coordinate closely with, T11 (handle
lifecycle) and T9 (sidebar file affordances) to avoid merge conflicts in
`ProjectPanel`/`+page.svelte`. The new `manifest.js` is conflict-free.
