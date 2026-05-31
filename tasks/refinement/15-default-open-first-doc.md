# T15 — Default to first doc, not first code file (#web-ide)

## Summary
When a workspace opens (folder, sample, recent, or future shared/imported), the IDE
should land on the **first doc page** declared in the manifest's `[[doc.sidebar]]`
(manifest order) instead of the first `.pds` module — falling back to the first code
file only when the workspace declares no readable doc pages. The "open this doc"
mechanism already exists (`openDoc(item)`); the change is which thing `mountWorkspace`
selects on load, plus making doc loading finish before that choice is made.

## Current state (file:line) — where initial active view is chosen, code vs doc selection
- **There is one "active" concept, not two panes.** The open thing is `openFile`
  (web-ide/src/routes/+page.svelte:72). A `.pds` module is `{ path, fqn, handle }`; a
  doc page is the shape `{ isDoc: true, path, title, handle }` set by `openDoc` (:606–611).
  The derived `source` (:147–153) reads `docSources[openFile.path]` when `openFile.isDoc`
  else `moduleSources[openFile.fqn]`; the editor drops PseudoScript features for an
  `isDoc` file. **There is no separate `activeDoc` / `docDraft` / dirty state** — opening
  a doc is just assigning `openFile` to an `isDoc` object.
- **`view`** (:28) is the right-pane mode `"code" | "canvas" | "problems"`. Docs are
  shown *in the `"code"` view* (the editor pane), so opening a doc sets `view = "code"`
  (:610). There is no `"doc"` view.
- **Initial selection lives in `mountWorkspace(ws, landing)` (:473–487):**
  - `openFile = ws.files.find(f => f.fqn === landing) ?? ws.files[0] ?? null` (:476) —
    always a **code** file (an FQN); never an `isDoc` page.
  - `view = "code"` (:479); doc state reset: `docGroups = []` (:483),
    `docSources = {}` (:484), `docMeta = {}` (:485).
  - `loadWorkspaceDocs(ws)` (:486) runs **async, after** the landing choice, so
    `docGroups`/`docSources` are still empty at the moment the landing is picked.
- **`loadWorkspaceDocs(ws)` (:493–515)** parses `ws.manifestToml` via `docManifest`
  (web-ide/src/lib/pds.js:181), then reads pages from disk (`readDocPages`,
  workspace.js:61) for a folder or from the bundled map (`sampleDocPages`, +page.svelte
  ~:692) for a sample. It sets `docSources` (path→Markdown, :507–509), `docMeta` (:510),
  and `docGroups` in **manifest order** (:511–514, each item `{ title, path, handle }`).
  A `docLoadSeq` token (:494, :506) discards a stale async load from a prior workspace.
- **Programmatic "open a doc" already exists:** `openDoc(item)` (:606), where `item` is
  `{ title, path, handle }` (a `docGroups[g].items[i]`). The FileTree Documentation
  rows trigger it via `ondocopen={openDoc}` on `<FileTree>` (:912, prop at :917).
- **The mount call sites (entry points to cover):**
  - `openSample` (:519) → `mountWorkspace(loaded.workspace, loaded.landing)` (:523)
  - `openRecent` folder branch (:530) → `mountWorkspace(ws, ws.files[0]?.fqn)` (:545)
  - `openFolder` (:625) → `mountWorkspace(ws, ws.files[0]?.fqn)` (:631)
  - recent→sample delegates to `openSample`.
- **`landing` is always a code FQN today.** Samples: `meta.landing` (a module FQN) or
  `null` (web-ide/src/lib/samples.js:64, :92). Folders/recents pass `ws.files[0]?.fqn`.
  No doc-aware landing exists anywhere.
- **`meta.json` `landing`** (samples.js:64) is read at sample-load time and handed to
  `mountWorkspace` as `landing`; it only ever names a code module today.
- **Share/import (T6/T7)** does not yet call `mountWorkspace` (grep finds only the three
  sites above). Putting the rule *inside* `mountWorkspace` means any future entry point
  inherits it.

## Proposed approach (selection rule + precedence with meta.json landing; all entry points)
Make `mountWorkspace` the single decision point and have it default to a doc:

1. **Await docs before choosing.** Have `loadWorkspaceDocs(ws)` `return` the built
   `groups` (it already computes them and sets state; just also return). In
   `mountWorkspace`, `await loadWorkspaceDocs(ws)` (mount becomes `async`) so
   `docGroups`/`docSources` are populated before the landing decision. Keep the
   `docLoadSeq` guard; if a newer mount superseded this one, `mountWorkspace` must
   re-check the seq (or simply bail) before assigning `openFile`/`view`, to avoid a
   stale load clobbering the current workspace.

2. **Default-selection rule (in `mountWorkspace`, after docs load):**
   - `firstDoc` = the first item of the first **non-empty** group, scanning groups in
     manifest order. Do **not** assume `docGroups[0].items[0]` exists — unreadable pages
     are dropped (workspace.js:68), so a declared group can end up empty.
   - **If `firstDoc` exists → open it:** `openDoc(firstDoc)` (reuse the existing fn; it
     sets the `isDoc` `openFile` and `view = "code"`). Still also resolve a code default
     into a holding var so the user has a sensible module when they pick one — but
     `openDoc` overwrites `openFile`, which is the desired landing.
   - **Else (no readable doc) → code fallback:** today's behaviour —
     `openFile = ws.files.find(f => f.fqn === landing) ?? ws.files[0] ?? null`,
     `view = "code"`.

3. **Precedence with `meta.json` `landing` — recommended: explicit `landing` wins, and
   it may name a doc.** Extend the `landing` contract: `landing` may be a doc page `path`
   or a module FQN.
   - If `landing` matches a loaded doc `path` → `openDoc(thatItem)`.
   - Else if `landing` matches a `files[].fqn` → open that code file (`view = "code"`).
   - Else (no/unknown `landing`) → first-doc default (rule 2), then code fallback.
   This keeps authored sample intent authoritative and defaults helpfully when unset.
   Folders/recents pass no meaningful `landing` (just `files[0].fqn`), so they get the
   first-doc default unless that FQN is genuinely the intended one — see open question.
   *Simpler alternative: first-doc default always wins; `landing` ignored for view.*

4. **All entry points free.** The rule lives in `mountWorkspace`, so
   `openSample`/`openRecent`/`openFolder` and any future share/import caller (T6/T7) get
   it. The three sites only need to tolerate `mountWorkspace` becoming `async`
   (`await`/`void` it; they already run in async contexts except `openSample`, which can
   `void` or be made async).

## Affected/new files
- **web-ide/src/routes/+page.svelte** — `mountWorkspace` (:473): make `async`, await
  docs, apply the doc-first selection rule (reuse `openDoc`, :606); `loadWorkspaceDocs`
  (:493): `return groups`. Call sites :523/:545/:631 adjust for the async signature.
- **web-ide/src/lib/samples.js** (:64, :92) — only if precedence rule #3 is adopted and
  `meta.landing` should be allowed to name a doc path (otherwise unchanged).
- **No change** to workspace.js (`readDocPages` already yields manifest-ordered groups)
  or pds.js (`docManifest`).
- Optional new: a sample `meta.json` doc-landing fixture to test rule #3.

## Open questions / decisions needed
- **Does explicit `meta.json` `landing` override the first-doc default?** Recommended:
  yes (rule #3). Alternative: first-doc always wins, `landing` only used for the code
  fallback. Choice decides whether samples.js changes.
- **What if `landing` points at code (current samples)?** Under rule #3 it opens that
  code file. Note: existing samples set `landing` to a module FQN, so adopting rule #3
  *as-is* would keep them on code, defeating the feature for samples. Decide one of:
  (a) treat a code `landing` as "author chose code, honour it"; or (b) only honour
  `landing` when it names a doc, else fall to first-doc default; or (c) audit/clear
  sample `meta.landing` values so the first-doc default fires. Recommended: (b).
- **No-doc workspace:** falls back to first code file (today's behaviour). Confirmed.
- **Async mount / flash of code view:** awaiting doc load before setting `openFile`
  avoids a code→doc flicker, but a slow folder read delays first paint. Option: paint the
  code landing immediately, then swap to the doc once `loadWorkspaceDocs` resolves (only
  if the user hasn't already navigated). Decide acceptable UX.
- **Unreadable first doc:** the rule scans for the first non-empty group — confirm OK.

## Dependencies on other tasks (T3 init/save, T4 folder-only docs, T6/T7 share-import, T10 new docs)
- **T4 (folder-only docs / preview links+images, 04-…):** T15 rides on `docGroups`/
  `docSources` being populated for folders; T4 governs folder doc discovery/ordering. If
  T4 changes source or order, T15's "first doc" follows it. Light coupling — coordinate.
- **T6/T7 (06 compressed-share-url / 07 import-export):** these add new workspace entry
  points. Because the rule is inside `mountWorkspace`, they inherit it *iff* they mount
  via `mountWorkspace`. Soft dependency: T6/T7 should route through `mountWorkspace`.
- **T10 (10 creating-new-doc-files):** adding a doc to a no-doc workspace changes which
  branch fires; no hard dependency — the landing rule should run only at mount, not on
  every doc add (a freshly-created doc shouldn't yank the view on an existing session).
- **T3 (03 init-workspace-save-disk):** `mountWorkspace` clears `saveTimer` (:474);
  `openDoc` also clears it (:607). Opening a doc on mount must not trigger a save. No hard
  dependency.

## Acceptance criteria (testable)
1. Opening a workspace whose manifest declares ≥1 readable `[[doc.sidebar]]` page lands
   with `openFile.isDoc === true` and `openFile.path` == the first declared (manifest-
   order) readable page; `view === "code"`.
2. Opening a workspace with **no** readable doc page lands with a code `openFile`
   (the resolved `landing` / `files[0]`) and `view === "code"` — today's behaviour.
3. The rule holds for all entry points: sample, folder, recent-folder (and recent→sample).
4. A future share/import path that mounts via `mountWorkspace` gets the same default with
   no extra code.
5. (If rule #3 adopted) A `meta.json` `landing` naming a doc path lands on that doc; the
   chosen behaviour for a code-FQN `landing` matches the decision in Open questions.
6. The opened doc's editor `source` equals `docSources[openFile.path]` (the page Markdown).
7. A workspace whose only declared doc page is unreadable falls back to code (criterion 2).

## Rough size (S/M/L) + parallel-safe?
**S–M.** Core logic is one function, but making `mountWorkspace` async, auditing the
three call sites, and handling the doc-load ordering/race needs care. **Parallel-safe-ish:**
the edit is confined to `mountWorkspace`/`loadWorkspaceDocs` in +page.svelte; it will
conflict textually with any task editing that region (T4 doc discovery, T6/T7 adding a
mount call site). Coordinate merge order with T4 and T6/T7; otherwise independent.
