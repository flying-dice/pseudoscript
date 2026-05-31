# T5 — Search bar (#web-ide)

## Summary (+ scope decision: in-file vs cross-file vs quick-open)

Add a search capability to the web IDE. Three candidate scopes:

- **(a) In-file find/replace** — search the open editor buffer.
- **(b) Cross-file (workspace) search** — text search across every loaded
  `.pds` module + Markdown doc, with a results list that jumps to file + line.
- **(c) Command palette / file quick-open** — fuzzy-jump to a file by name.

**Decision — MVP is (a) in-file find via the official `@codemirror/search`
package; (b) cross-file workspace search is the strong stretch.** Rationale:

- The editor is CodeMirror 6 (verified). CM6 ships a complete, accessible
  find/replace panel in `@codemirror/search` — open panel, next/prev, match
  count, regex/case/whole-word, replace, highlight. It is **not currently
  installed or wired** (verified: absent from `package.json`, absent from
  `Editor.svelte` imports). Adding it is one dependency + one keymap entry +
  one shortcut-catalogue entry. Highest value per unit effort, and it is the
  reflexive `Cmd/Ctrl-F` behaviour users expect.
- (b) cross-file search is the bigger architectural win for a *workspace* IDE
  (sample workspaces hold ~13 `.pds` + docs each). Crucially, **the codebase
  already has every primitive it needs** — a workspace file set, a
  jump-to-file-and-line path, and a "find usages" results-panel pattern to
  clone (see Current state). So the stretch is M, not L.
- (c) quick-open is lower priority for an architecture tool whose payoff is
  finding a node/edge across modules, not opening a file by name. Defer.

So: **MVP = (a); Stretch = (b); Later = (c).**

## Current state (file:line) — editor lib + what it gives free

Editor library is **CodeMirror 6** — `web-ide/package.json:20-33` lists the
`@codemirror/*` packages and `codemirror`. **`@codemirror/search` is not among
them** (the built-in find panel is not wired in — the single dependency the MVP
adds).

`web-ide/src/lib/components/Editor.svelte` (CM6 host):
- Keymap is built in two layers in `onMount`, `Editor.svelte:573-617`:
  - `keysCompartment.of(shortcutKeymap())` first (highest precedence), then
    `keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap,
    ...foldKeymap, indentWithTab])` (`Editor.svelte:581-588`).
  - `searchKeymap` slots into that second `keymap.of([...])` array; any custom
    search command (e.g. open cross-file search) goes in the `shortcutRun`
    catalogue at `Editor.svelte:524-535` so it's user-rebindable.
- Customisable-shortcut catalogue: `shortcutRun` maps command id → CM command
  (`Editor.svelte:524-533`); `shortcutKeymap()` (`:534-535`) builds the keymap
  from `keybindings.keyFor(id)`. **`keybindings.svelte.js` exists** at
  `web-ide/src/lib/keybindings.svelte.js` (imported `Editor.svelte:20`) — the
  brief's assumption that it's missing is wrong.
- Existing nav primitives reusable by cross-file search:
  - `goto(line, col)` (`Editor.svelte:436-450`) — moves cursor to a 1-based
    line/byte-col, scrolls into view, flashes. Exposed via `onready` as
    `editorApi.goto` (`:623`).
  - A complete **find-usages results panel** (`Editor.svelte:659-688` markup +
    `:705-771` styles) — a portalled, viewport-anchored list of
    `path:line` + match-preview rows. This is the template to clone for a
    cross-file results panel.

`web-ide/src/routes/+page.svelte` (shell) — file-set shape + open/jump path:
- In-memory file set is verified: `moduleSources` is `{ fqn: sourceString }`
  (`+page.svelte:92`), `docSources` is `{ path: markdownString }` (`:97`);
  `allModules` = `[{ fqn, source }]` (`:137-139`) — the same shape every wasm
  API takes. Doc pages live in `docGroups` (`:96`).
- Open a file + jump: `selectFile(file)` (`:577-582`), `selectNode(fqn,{goto})`
  (`:371-388`), and the `pendingGoto` → `editorApi.goto` effect (`:429-435`).
- **A cross-file jump already exists**: `openUsage(occ)` → `applyLocation(loc)`
  (`:67-86`) opens the occurrence's file and jumps the editor to its line/col,
  recording it in history — exactly what a search-result click needs.

`web-ide/src/lib/workspace.js` — `readWorkspace` returns
`{ name, root, base, manifestToml, files }` where `files` is
`[{ path, fqn, handle }]` (`workspace.js:42-50`, typedef `:124-126`). No
all-files-text helper is needed for the stretch: `moduleSources` + `docSources`
in the shell already hold every file's live text.

`web-ide/src/lib/pds.js` — `references(modules, moduleFqn, offset)`
(`pds.js:127-129`) already returns cross-workspace occurrences
(`{ fqn, line, col, text, … }`); useful precedent, and the symbol-aware search
follow-up would extend from here.

What CodeMirror gives free once `@codemirror/search` is added: `search({ top:
true })` panel, commands `openSearchPanel`/`closeSearchPanel`/`findNext`/
`findPrevious`/`replaceNext`/`replaceAll`, `searchKeymap` (Mod-f, Mod-g,
Shift-Mod-g, …), match highlighting + count, regex/case/whole-word toggles.

## Proposed approach (MVP + stretch)

**MVP — in-file find/replace (S):**

1. Add dependency `@codemirror/search`.
2. In `Editor.svelte`, add `search({ top: true })` to the extensions list and
   merge `...searchKeymap` into the existing `keymap.of([...])`
   (`Editor.svelte:582-588`) — after `defaultKeymap`, before `indentWithTab`.
3. Optional: an "openSearch" entry in the `shortcutRun` catalogue
   (`:524-533`) calling `openSearchPanel(view)`, so it shows in Settings and is
   rebindable; plus an optional Find button in `Toolbar.svelte`.
4. Theme the panel via `EditorView.theme` (the same place the autocomplete and
   fold-pill overrides already live, `Editor.svelte:247-362`) so it matches the
   dark IDE instead of CM's default light panel.

No new component, no index, no jump wiring — all provided by the package.

**Stretch — cross-file workspace search (M):**

1. New `SearchPane.svelte` (sibling of `ProblemsPane.svelte`), OR a fourth tab
   in the existing `viewToggle` (`+page.svelte:834-842`). Query input +
   results grouped by file, each row `path:line` + match preview — clone the
   find-usages panel markup/styles from `Editor.svelte:659-771`.
2. Search source: scan the shell's `moduleSources` (and optionally
   `docSources`) — both already in memory. Per-file `String.matchAll`/regex
   line scan; no index needed at this corpus size (tens of files). Add
   case/regex toggles for parity with the in-file panel.
3. Jump-to-match: build a `loc` and call the **existing** `applyLocation` /
   `openUsage` path (`+page.svelte:67-86`) — the file-open + `editorApi.goto`
   plumbing is already there and history-aware. This is the one new wire, and
   it's a reuse, not a build.
4. Trigger: a toolbar button + `Shift-Mod-f` (added to `shortcutRun`).

**Later — file quick-open / command palette (M):** modal fuzzy-filter over
`workspace.files[].fqn`; `Mod-p`. Independent of the above.

## Affected/new files

- `web-ide/package.json` — add `@codemirror/search` (MVP).
- `web-ide/src/lib/components/Editor.svelte` — search extension + `searchKeymap`
  + optional `shortcutRun` entry + panel theme (MVP).
- `web-ide/src/lib/keybindings.svelte.js` — add the `openSearch` (and stretch
  `openWorkspaceSearch`) command ids to the catalogue (MVP/stretch).
- `web-ide/src/lib/components/Toolbar.svelte` — optional Find button (MVP).
- `web-ide/src/lib/components/SearchPane.svelte` — **new** (stretch).
- `web-ide/src/routes/+page.svelte` — mount SearchPane / add tab, feed it
  `moduleSources`+`docSources`, route result clicks through `applyLocation`
  (stretch).

## Open questions / decisions needed

- Stretch placement: a 4th tab in the Code|Canvas|Problems toggle vs. a left
  side panel. The tab reuses the most existing structure.
- Should cross-file search also match **model symbols** (node/edge names) via
  the wasm model, not just raw text? That's the architecture-aware
  differentiator, but needs a wasm symbol-enumeration API (today only
  `references()` exists, keyed off a cursor offset). Treat as a follow-up beyond
  the text stretch.
- Include Markdown `docSources` in cross-file results, or `.pds` only? Recommend
  including docs (cheap, and the corpus is small).
- Confirm `searchKeymap`'s `Mod-f` does not collide with any existing
  `shortcutRun` binding (none of the current ids use `Mod-f`, verified at
  `Editor.svelte:524-533`).

## Dependencies on other tasks

- **MVP: none.** One npm dep + Editor wiring; self-contained.
- **Stretch** reuses `applyLocation`/`openUsage` and the `moduleSources`/
  `docSources` state in `+page.svelte`. If another task is restructuring that
  shell state or the view-toggle, land the stretch after it.
- Symbol-aware search (follow-up) depends on a new wasm enumeration API in
  `pds.js` — its own backlog item.

## Acceptance criteria (testable)

MVP:
- `Cmd/Ctrl-F` in the editor opens a find panel.
- Typing highlights all matches with a count; Enter / `Mod-g` cycles next,
  `Shift-Mod-g` previous.
- Replace and Replace-all work and undo as one step.
- Regex, case-sensitivity, whole-word toggles function.
- `Esc` closes the panel, clears highlights, returns focus to the editor.
- Panel matches the dark IDE theme (no default light box).

Stretch:
- A workspace-search input returns matches across all loaded `.pds` (and docs),
  grouped by file with `path:line` + preview.
- Clicking a result opens that file and places the cursor on the matched line,
  scrolled into view (via the existing jump path), and records it in nav history.
- Case and regex toggles affect results; empty query clears; zero matches shows
  an empty state.

## Rough size (S/M/L) + parallel-safe?

- **MVP: S.** One dependency + a few lines in `Editor.svelte` (+ optional
  catalogue/Toolbar/theme). Parallel-safe — touches only Editor/keybindings/
  Toolbar.
- **Stretch: M** (not L — jump + file-set primitives already exist; it's a new
  panel + a reuse of `applyLocation`). Coordinate only if another task is
  editing `+page.svelte` shell state or the view-toggle.
