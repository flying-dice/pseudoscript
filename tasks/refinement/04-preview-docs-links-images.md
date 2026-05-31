# T4 — Fix doc preview links/images; folder-only docs (#web-ide)

## Summary

In the web IDE, when a workspace's `docs/*.md` files are opened in the
Markdown live-preview editor, relative links between docs (`[other](other.md)`)
and embedded images (`![logo](../logo.svg)`, `![diagram](images/foo.png)`)
are broken. The live preview is a CodeMirror decoration layer over the raw
Markdown text (`web-ide/src/lib/markdown-live.js`); it renders link/image
*spans* but never resolves their relative URLs against the doc's location, and
images are not rendered as `<img>` at all. For sample / in-memory workspaces
there is no backing directory, so a relative path can never be resolved to real
bytes. Baked-in decision: resolve relative links/images only when the workspace
is backed by a real folder (a `FileSystemDirectoryHandle` / file URLs);
otherwise hide/disable the docs preview entirely (no broken links, no
"missing image" boxes).

## Current state (file:line) — why links/images break

CONFIRMED (files read in full this session):

- `web-ide/src/lib/markdown-live.js` — the entire doc-preview engine. It is a
  CodeMirror 6 live-preview decoration set, NOT an HTML renderer:
  - Links: `markdown-live.js:69` registers `Link: Decoration.mark({ class:
    "cm-md-link" })` and `:73` hides the `URL`/`LinkMark` markers. The link is
    only *styled* — no `href`, no click handler, no resolution of the target.
    So `[a](b.md)` shows styled text that goes nowhere. (Inside GFM table cells,
    `renderInline` at `:232–:241` DOES set an `href`, but only for absolute /
    `/` / `#` / `mailto` targets — `:237` `/^(https?:|mailto:|\/|#)/i` — i.e.
    relative `.md`/image links are deliberately dropped even there.)
  - Images: NO `Image` handling anywhere in the decoration walker (`decorate`,
    `markdown-live.js:255–401`). `![alt](src)` is left as raw Markdown text; no
    `<img>` is ever produced, and no URL resolution exists.
  - The engine has no notion of "where this doc lives": `markdownLivePreview()`
    (`:520–:538`) takes no base-path / workspace context, so even if it rendered
    images it could not resolve a relative `src`.

- `crates/pseudoscript-doc/src/url.rs` — the CLI's URL scheme, for mirroring.
  `doc_page_path` (`:79–:85`) flattens `docs/<...>.md` → `docs/<slug>.html` and
  slugs filenames (`docs/guides/configuration.md` →
  `docs/guides-configuration.html`). So an authored relative link
  `[x](./sibling.md)` is NOT rewritten by the CLI either.

- `crates/pseudoscript-doc/src/render.rs:94–104` — `render_markdown` runs
  `pulldown_cmark` (GFM) and emits HTML **verbatim** for links and images: it
  does not rewrite relative `href`/`src`. In the static site this "works" only
  because authors use paths that resolve relative to the output `docs/` dir.
  Confirms: the fix is net-new resolution logic, not porting an existing
  resolver — the CLI side is also naive.

- `crates/pseudoscript-doc/src/assets.rs:1–14` — only embeds `ssr.js`,
  `client.js`, `style.css`. No image/asset-copying logic here (logo handling is
  in `config.rs`/`site.rs`). No existing image resolver to reuse.

- `web-ide/src/lib/components/Editor.svelte:19,542-553` — the single mount
  point. `languageBundle()` returns `markdownLivePreview()` (no arguments) when
  the `markdown` prop is set (`:543`). The component receives `value`/`onchange`
  but NOT the active file's workspace-relative path, nor any workspace handle —
  so today the editor literally cannot know where the doc lives or reach sibling
  files. Both must be threaded in.

- `web-ide/src/lib/workspace.js` — the folder-backed model. A real workspace
  carries `{ name, root: FileSystemDirectoryHandle, base, manifestToml, files }`
  (`:50`); each file has a live `handle` (`:44`). `openFileAt` (`:75-87`) is
  exactly the directory-walk a resolver needs (split on `/`, `getDirectoryHandle`
  per part, `getFileHandle`). `readFile` (`:90-93`) yields a `File` (→
  `URL.createObjectURL`). No relative-path/`..` resolution exists yet.

- `web-ide/src/lib/samples.js:78-94` — the non-folder signal, confirmed.
  `loadSample` returns a workspace with `root: null` and each file `handle: null`
  (`:46`), plus an in-memory `docs` map keyed by sample-relative path
  (`:52-55,:88`). So `workspace.root == null` (or `file.handle == null`) is the
  reliable `isFolderBacked === false` test. Samples DO ship docs as in-memory
  strings (acme-tickets has 6: overview/contexts/payments/surge/checkout-saga/
  edge-cases.md), so relative links between them could in principle resolve from
  the `docs` map even without a folder — but the baked-in decision is to gate on
  a folder, so sample docs preview is disabled regardless.

## Proposed approach (fix + folder-gating)

Two coordinated changes — a capability gate, and (only when capable) actual
relative-URL resolution.

1. Workspace capability flag (`workspace.js`):
   - Add a derived `isFolderBacked` (true when opened via the File System
     Access API with a directory handle / real file URLs; false for sample /
     in-memory workspaces from `samples.js`).
   - Expose `resolveDocAsset(docPath, relTarget)`: given the active doc's
     workspace-relative path and a relative link/image target, walk the
     directory handle to the target file and return either an in-app navigation
     target (`.md` → open that doc) or `URL.createObjectURL(blob)` (images).
     Returns `null` when the target is missing or the workspace is not
     folder-backed. Track and `URL.revokeObjectURL` object URLs on doc
     swap/unmount.

2. Live-preview resolution (`markdown-live.js`):
   - Parameterise `markdownLivePreview(ctx)` with `{ resolveLink, resolveImage }`
     (no-ops when absent). Default `ctx = {}` so existing callers are
     unaffected.
   - Images: add an `Image` branch in `decorate` that, when `ctx.resolveImage`
     yields a URL, replaces the `![alt](src)` span with an `<img>` widget
     (reveal raw source when the cursor is on the line, matching the
     table/rule/fence reveal pattern). When unresolved, leave raw text (no
     broken-image box).
   - Links: give `cm-md-link` an `href`/click handler. `http(s)`/`mailto`/`/`/`#`
     keep current behaviour. Relative targets call `ctx.resolveLink`; a resolved
     `.md` opens that doc in the IDE, a resolved asset opens the object URL;
     unresolved relative links render as inert text (mirrors `url.rs` §9.3
     "unresolved → plain text").

3. Folder-gating the docs preview (mounting component — DiagramPane / Editor /
   ProjectPanel):
   - When `!isFolderBacked`, mount `markdownLivePreview` without a resolver and
     hide/disable the rendered docs preview for `.md` (the file may still open
     as plain editable Markdown; show a one-line notice like "Open this project
     from a folder to preview docs"). Recommend keeping sample `.md` files
     visible/editable (teaching content) and disabling only the rendered
     preview.

Mirror CLI semantics where sensible: relative `.md` link resolution should map
to the same target identity the CLI emits (`doc_page_path`), so IDE preview and
`pds doc` stay consistent.

## Affected/new files

- `web-ide/src/lib/markdown-live.js` — add `Image` widget + render branch; add
  link `href`/click + relative resolution hook; thread `ctx` through
  `markdownLivePreview` and `decorate`.
- `web-ide/src/lib/workspace.js` — add `isFolderBacked`, `resolveDocAsset`,
  object-URL lifecycle. (CONFIRM model shape first.)
- `web-ide/src/lib/components/Editor.svelte` — accept the active doc's
  workspace-relative `path` and a resolver context as props; pass them into
  `markdownLivePreview(ctx)` at `:543`. (CONFIRMED sole mount point.)
- The page/shell that renders `<Editor markdown>` (the `+page.svelte` owning the
  workspace state) — derive `isFolderBacked` from `workspace.root != null`, pass
  the resolver + active path down, and gate/hide the docs preview affordance.
- `web-ide/src/lib/samples.js` — no change needed; `root: null` /
  `handle: null` already signals not-folder-backed (CONFIRMED).
- New files: none expected; extend existing modules.

## Open questions / decisions needed

- The active doc's workspace-relative path does NOT reach `Editor.svelte` today
  (it only gets `value`/`onchange`). Threading the path in is required —
  decide whether to add a `path` prop to `Editor` or compute the resolver in the
  shell and pass it pre-bound to the current doc. (Recommend: shell binds a
  per-doc resolver, Editor stays path-agnostic.)
- Sample-docs UX when not folder-backed: hide `.md` from the tree, or keep
  editable but disable rendered preview? (Recommend: keep + disable preview.)
- Relative `.md` links: open the target doc in-app, or render styled-but-inert?
  (Recommend: in-app open when folder-backed.)
- Anchor links within a doc (`[x](#heading)`) — keep working client-side
  regardless of folder backing? (Recommend yes.)
- CLI parity: also rewrite relative `.md` links in `render.rs` to the flattened
  `doc_page_path` form, or out of scope for this IDE-only ticket? (Likely a
  separate follow-up.)

## Dependencies on other tasks (T3 init/save, T10 new doc files, T12 markdown formatting)

- T3 (init/save): folder-backed write/save establishes the directory-handle
  plumbing this task reads from; `isFolderBacked` should reuse T3's handle.
  Order T3 before, or share the handle abstraction.
- T10 (new doc files): new `.md` docs must land in the folder so relative links
  resolve; T10 and T4 share the docs/ directory model. T10 first lets T4 resolve
  against real files immediately.
- T12 (markdown formatting): edits the same `markdown-live.js` decoration set.
  Coordinate to avoid conflicts in `decorate()` — both touch the same `enter`
  switch.

## Acceptance criteria (testable)

- Folder-backed: `![x](relative.png)` and `![x](../logo.svg)` render as visible
  `<img>` in the preview; raw `![...]()` reveals on the cursor line for editing.
- Folder-backed: `[other](other.md)` resolves and opens that doc in the IDE;
  `[x](#heading)` scrolls within the doc; `https://…` opens externally as today.
- Folder-backed: a relative link/image to a non-existent file renders as inert
  styled text / leaves raw `![…]()` (no broken-image icon, no JS error).
- Non-folder (sample/in-memory): the rendered docs preview is hidden/disabled
  per chosen UX; no broken links/images shown; `.md` may still open as plain
  Markdown.
- No object-URL leaks: image object URLs revoked on doc switch / unmount.
- Existing `.pds`-adjacent uses of `markdownLivePreview()` unaffected (default
  empty `ctx`).

## Rough size (S/M/L) + parallel-safe?

M. Two surfaces (workspace resolver + CM decoration branches) plus a UI gate.
Self-contained within `web-ide`; no Rust changes needed for the IDE fix.
Parallel-safe against most tasks EXCEPT T12 (same `markdown-live.js`
`decorate()` switch) and loosely T3/T10 (share the directory-handle model) —
sequence or coordinate those.
