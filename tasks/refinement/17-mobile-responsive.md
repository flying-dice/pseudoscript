# T17 — Mobile support / responsive IDE (#web-ide)

## Summary (+ realistic mobile MVP scope)

The web IDE (`web-ide/`, a SvelteKit SPA) is built desktop-first: a three-row
grid (`topbar / content / statusbar`) with a fixed two-column workspace
(`268px` sidebar + content pane) that never collapses, a side-by-side editor and
diagram model, and several affordances that only exist on a desktop pointer +
keyboard (hover tooltips, Cmd/Ctrl-click, custom keymaps, the File System Access
folder picker). The viewport meta tag is present and correct, so pages *render*
on a phone, but the layout overflows: the toolbar's seven flex children, the
hard-coded sidebar column, and the pointer-anchored popovers all assume a wide
screen.

There is no responsive CSS anywhere in the app shell. The **only** media queries
in the whole codebase are `prefers-reduced-motion` (`app.css:356`) and a single
`max-width: 720px` grid collapse inside the ProjectPanel modal
(`ProjectPanel.svelte:223`). Everything else is fixed.

**Realistic mobile MVP** (what a phone genuinely supports):
- Browse + open the bundled **examples** (in-memory, no FS Access needed —
  `samples.js` / `loadSample`).
- **Read & edit** `.pds` source and Markdown docs in a single full-width pane
  (CodeMirror is already touch-capable).
- **View diagrams** (C4 + sequence) in the same pane, switched via tabs rather
  than shown alongside the editor — Svelte Flow supports pinch-zoom/drag.
- Navigate the file/symbol tree from a slide-over drawer.
- See and tap problems to jump to source.

**Out of MVP (desktop-only, degrade gracefully):** opening an on-disk **folder**
workspace (File System Access API — Chromium desktop only; `workspace.js:10`
already feature-gates this via `fsSupported`), and the keyboard-shortcut
customisation surface. Build-to-disk also depends on FS Access; the in-tab
preview still works.

## Current state (file:line)

**Viewport meta — present & correct.** `app.html:6`
`<meta name="viewport" content="width=device-width, initial-scale=1" />`. No
`viewport-fit=cover`, so no safe-area handling for notched phones.

**App shell — fixed grid, no breakpoints.** `+page.svelte:992`
- `.app` is `grid-template-rows: var(--topbar-h) 1fr var(--status-h)` with
  `height: 100vh` (`+page.svelte:993-996`). `100vh` is the wrong unit on mobile
  (excludes/includes the dynamic browser chrome inconsistently → bottom
  statusbar gets cut off or scrolls); should be `100dvh`.
- `.workspace.has-tree { grid-template-columns: 268px minmax(0,1fr); }`
  (`+page.svelte:1002-1004`) — the sidebar is a hard `268px` column with **no
  collapse rule**. On a 360px phone that leaves ~92px for the editor.
- `.content-bar` (`+page.svelte:1026-1033`) is a single non-wrapping flex row
  holding back/forward buttons + breadcrumb + the view toggle; it will overflow
  horizontally on narrow screens.

**Topbar — overflows, hover-dependent label.** `Toolbar.svelte:52-61`
- `.toolbar` is `display:flex; gap:1rem; height:60px` with seven children
  (brand, project button, spacer, hint `<p>`, build button, settings button,
  Format button, status). No wrap, no `min-width:0` on the row; the project
  button alone is `max-width:18rem`. Total intrinsic width far exceeds a phone.
- `.hint` "Hover a symbol for its diagram" (`Toolbar.svelte:27,111`) is
  meaningless on touch (no hover) and pure horizontal cost.
- `--topbar-h: 60px` (`app.css:64`) is a generous fixed bar; fine, but combined
  with the statusbar it eats vertical space that matters more on mobile.

**Editor — desktop interaction model.** `Editor.svelte`
- Hover tooltip with the symbol diagram (`Editor.svelte:551`,
  `hoverTooltip(symbolTooltip,{hoverTime:250})`) — **no touch equivalent**;
  this is the IDE's headline feature and is unreachable on a phone.
- Cmd/Ctrl-click for go-to-def / find-usages
  (`Editor.svelte:596-605`, `gotoLinkPlugin` `Editor.svelte:189-245`) — relies on
  a modifier key + mouse; absent on touch.
- The find-usages dropdown is `position:fixed` anchored to viewport coords
  (`Editor.svelte:705-716`, `max-width:min(34rem,90vw)`) — already viewport-aware,
  acceptable on mobile.
- CodeMirror itself is touch-capable (scroll, caret, selection handles), so
  plain editing works once the pane is full-width.

**Diagram pane — usable, controls small.** `DiagramPane.svelte`,
`app.css:138-223`
- Svelte Flow supports pinch-zoom / drag pan natively; the depth selector
  (`DiagramPane.svelte:61-73`) and Flow controls are small but functional.
- Canvas hover-info + usages popovers are pointer-anchored
  (`+page.svelte:774-795`, `showCanvasInfo` uses `e.clientX/clientY`) — on touch
  these fire on tap but anchor near the finger; workable, not ideal.

**FileTree — touch targets too small, no drawer.** `FileTree.svelte`
- Row paddings are tight: `.file` `padding:0.34rem 0.5rem`
  (`FileTree.svelte:250`), `.node` `padding:0.22rem 0.5rem`
  (`FileTree.svelte:304`), the collapse `.twist` is `1.1rem × 1.1rem`
  (`FileTree.svelte:278-280`) — all below the ~44px touch-target guideline.
- It lives in a permanently-visible column; on mobile it must become a
  toggleable drawer.

**ProjectPanel — partially responsive.** `ProjectPanel.svelte:223`
has the only real app breakpoint (`@media (max-width:720px)` collapses the
two-column grid to one). But the dossier is `width:min(64rem,100%)` with
`padding:1.9rem 2rem 2rem` (`ProjectPanel.svelte:140-146`) and
`max-height:calc(100vh-4rem)` + `overflow:hidden` — on a short phone the example
catalogue can be clipped (it relies on the inner `.col.examples`
`overflow-y:auto`, `:239-245`, which is fine, but the outer `100vh` is again the
wrong unit).

**Settings (keymap) modal — desktop-only by purpose.** `Settings.svelte:127-142`
`width:min(34rem,92vw)` is fine to display, but the entire feature (rebinding
keyboard chords) is irrelevant on a touch device.

**Popovers / modals — viewport-aware, mostly OK.** Notifications
(`Notifications.svelte:24-34`, `width:min(360px,calc(100vw-1.8rem))`) and the
build-notice modal (`+page.svelte:1250`, `width:min(460px,100%)`) already clamp
to the viewport.

## Desktop-only features that break on mobile

| Feature | Where | Mobile behaviour | Plan |
|---|---|---|---|
| File System Access (open folder, save-to-disk, build-to-disk) | `workspace.js:10-22,96-113`; gated by `fsSupported` | `window.showDirectoryPicker` undefined on iOS/Android browsers | Already feature-detected — the "Open a folder" button is disabled with an explanatory note (`ProjectPanel.svelte:90-96`). Keep; lean on samples + (stretch) share-URL. |
| Editor hover tooltip (symbol + diagram) | `Editor.svelte:551` | No hover event on touch — feature unreachable | Add a tap/long-press path, or surface the same info via tapping a symbol → Canvas (the existing `revealSymbol`). |
| Cmd/Ctrl-click navigation | `Editor.svelte:189-245,596-605` | No modifier+mouse | Provide an on-screen action (long-press context menu, or a "Go to def / Usages" affordance). Selection-based commands still reachable via a toolbar. |
| Keyboard shortcuts (all of `keybindings.svelte.js`) | `Editor.svelte:524-535` | No physical keyboard | Acceptable to leave; ensure every shortcut-only command also has a tappable UI entry (Format already has a button; go-to-def/usages need one). |
| Side-by-side editor + diagram | `+page.svelte:938-965` (already a tab swap, not split) | Two fixed columns squeeze | Current code already uses a **view toggle** (Code/Canvas/Problems share one cell), so no split to undo — just make the toggle + tree fit. |
| Drag-drop (T11) | not yet implemented | n/a today | When added, needs Pointer Events / touch DnD (note for T11). |
| `100vh` height | `+page.svelte:995`, `ProjectPanel.svelte:141`, several `max-height:*vh` | Mobile URL bar resize cuts off statusbar / clips modals | Switch to `100dvh` / `dvh` units. |

## Proposed approach

**Breakpoints (add as design tokens in `app.css :root`, co-design with T16):**
- `--bp-mobile: 640px` — single-pane, drawer nav, tabbed editor/diagram.
- `--bp-tablet: 1024px` — sidebar may overlay or narrow; toolbar condenses.
- Desktop ≥1024px — current layout unchanged.

**Layout — stack/tab instead of side-by-side.**
- Replace `height:100vh` with `100dvh` on `.app` (and the `*vh` modals).
- Below `--bp-mobile`, drop the `268px` sidebar column: `.workspace.has-tree`
  becomes a single `minmax(0,1fr)` column; the FileTree renders in a **slide-over
  drawer** (off-canvas `position:fixed`, toggled by a new hamburger button in the
  toolbar, with a scrim + Escape/back-button close, mirroring the existing modal
  pattern).
- The Code / Canvas / Problems **view toggle already is the tab model** — keep
  it, just ensure `.content-bar` wraps (`flex-wrap:wrap`) or moves the toggle to
  a bottom tab bar on mobile.

**Mobile nav.**
- Add a hamburger/menu button (left of brand) that opens the FileTree drawer.
- Consider a fixed bottom tab bar on phones carrying Code / Canvas / Problems +
  a "Files" button, freeing the top bar; or simply let the existing toggle wrap.
- Project button stays in the top bar (collapses to an icon below `--bp-mobile`).

**Toolbar condensation (`Toolbar.svelte`).**
- Hide `.hint` below `--bp-tablet` (`display:none`) — it's hover-only advice.
- Collapse text buttons to icon-only below `--bp-mobile` (Build → ⚙, Format → an
  icon or move into an overflow "⋯" menu); the project button already truncates.
- Let `.toolbar` wrap or push secondary actions into an overflow menu so it never
  overflows horizontally.

**Touch targets.**
- Bump interactive rows to ≥40px min-height under the mobile breakpoint:
  FileTree `.file` / `.node` / `.twist`, the `.nav-btn` back/forward
  (`+page.svelte:1035`, currently `1.7rem`), view-toggle buttons, depth selector.
- Add a touch-target token (`--tap-min: 44px`) and apply via the breakpoint.

**Capability degradation plan (explicit).**
- *Always works (any browser):* open examples, edit `.pds`/Markdown in memory,
  view C4 + sequence diagrams, problems list, format. Edits are session-only on
  examples regardless of platform (`scheduleSave` no-ops without a handle —
  `+page.svelte:579-583`).
- *Desktop-only (degrade with a clear note, already partly done):* open on-disk
  folder, save-to-disk, build-to-disk site. The ProjectPanel note already
  explains this (`ProjectPanel.svelte:95`).
- *Stretch as a mobile workspace path:* a **share-URL / import** mechanism (see
  T6/T7) lets a phone load a model without FS Access — the natural mobile
  substitute for "open folder".
- *Drop on mobile:* keymap customisation surface (no keyboard), hover-diagram as
  the *primary* discovery path (provide a tap path instead).

**MVP vs stretch.**
- **MVP:** `100dvh` fix; sidebar → drawer below `--bp-mobile`; toolbar condense
  (hide hint, icon-only/overflow); `.content-bar` wrap; touch-target sizing;
  verify CodeMirror + Svelte Flow gestures; safe-area insets
  (`viewport-fit=cover` + `env(safe-area-inset-*)`).
- **Stretch:** tap/long-press to reveal symbol info (hover replacement); bottom
  tab bar; share-URL workspace as the mobile "open" path; on-screen go-to-def /
  find-usages actions.

## Affected/new files

- `web-ide/src/app.html` — add `viewport-fit=cover` to the viewport meta.
- `web-ide/src/app.css` — breakpoint + tap-target tokens; `dvh` for any `*vh`;
  safe-area inset usage. (Co-design tokens with T16.)
- `web-ide/src/routes/+page.svelte` — `100dvh`; responsive `.workspace` /
  `.content-bar`; wire up drawer open/close state.
- `web-ide/src/lib/components/Toolbar.svelte` — hamburger button, condense /
  overflow, hide hint, icon-only buttons.
- `web-ide/src/lib/components/FileTree.svelte` — drawer presentation +
  larger touch targets (or a new `Drawer.svelte` wrapper).
- `web-ide/src/lib/components/Editor.svelte` — (stretch) tap/long-press path for
  hover-info and go-to-def/usages.
- `web-ide/src/lib/components/DiagramPane.svelte` — larger depth-selector touch
  targets.
- `web-ide/src/lib/components/ProjectPanel.svelte` — replace `100vh` with `dvh`;
  verify catalogue scroll on short phones.
- *(new, optional)* `web-ide/src/lib/components/Drawer.svelte` — reusable
  off-canvas drawer if the FileTree drawer is extracted.

## Open questions / decisions needed

1. **Target devices** — minimum width to support (360px? 320px?) and whether
   tablets get a distinct mid layout or just inherit desktop.
2. **Read-only vs editable on mobile** — confirm samples are editable in-session
   on mobile (they are, in memory) and that we don't promise persistence without
   FS Access. Is a read-only "viewer" mode acceptable as a first cut?
3. **FS Access fallback** — adopt the share-URL/import path (T6/T7) as the mobile
   workspace mechanism, or accept "examples only" for MVP?
4. **Hover replacement** — long-press vs tap-to-select-then-action for symbol
   info and go-to-def; needs a UX decision (and a tap path so the headline
   diagram-on-hover feature isn't lost on mobile).
5. **Bottom tab bar vs wrapping toggle** — pick one nav pattern for the
   Code/Canvas/Problems switch on phones.
6. **Drawer vs overlay sidebar** — full off-canvas drawer or a narrowed
   collapsible column on tablets.

## Dependencies on other tasks

- **T16 (styling / design tokens)** — co-design the breakpoint and tap-target
  tokens in `app.css :root` so both tasks share one source of truth; avoid
  conflicting edits to `app.css`.
- **T6 / T7 (share / import)** — the share-URL / import path is the realistic
  mobile substitute for "open folder" (which needs FS Access); mobile MVP can
  ship on examples alone but lands its full value once T6/T7 provide a
  no-FS-Access workspace path.
- **T11 (drag-drop)** — any DnD added there must use Pointer/touch events to work
  on mobile; flag mobile touch DnD as a requirement of T11.

## Acceptance criteria (testable across breakpoints)

1. At ≤640px width, the FileTree is **not** a permanent column; a menu/hamburger
   control opens it as a drawer, and a scrim/Escape/back closes it.
2. At ≤640px, the toolbar fits without horizontal overflow (no horizontal
   scrollbar on `.toolbar` / `.app`); the hover hint is hidden.
3. The bottom statusbar is fully visible (not clipped by mobile browser chrome)
   on iOS Safari and Android Chrome — i.e. `dvh`, not `vh`.
4. The editor pane is full-width on mobile (not ~92px); typing, caret placement,
   and selection work via touch.
5. Code / Canvas / Problems are switchable on mobile and each fills the content
   area; the Svelte Flow diagram supports pinch-zoom and drag-pan.
6. All primary interactive targets (file rows, symbol rows, tabs, nav buttons,
   depth selector) are ≥40px in their smallest dimension under the mobile
   breakpoint.
7. On a browser without File System Access, "Open a folder" / "Build docs" are
   disabled (or routed to the in-tab preview) with a visible explanation, and
   opening an **example** + editing it in-session works.
8. Notched-device safe areas are respected (no content under the status bar /
   home indicator) via `viewport-fit=cover` + `env(safe-area-inset-*)`.
9. Desktop (≥1024px) layout and behaviour are unchanged (regression check).

## Rough size + parallel-safe?

**Size: M** (MVP). Layout, drawer, toolbar condensation, unit/touch fixes are
mostly contained CSS + a little drawer state in `+page.svelte` and `Toolbar`.
**L** if the stretch items (tap/long-press hover replacement, on-screen
nav actions, share-URL mobile path) are pulled in.

**Parallel-safe: partially.** It edits many `web-ide` files but few of them
overlap other in-flight web-ide tickets — except **`app.css`**, which T16 also
touches: coordinate or sequence those two. Otherwise independent of the spec /
conformance / Rust work.
