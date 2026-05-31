# T12 — Markdown file formatting (#web-ide)

## Summary (+ scope decision: render vs source-format)

The title is ambiguous between (a) **rendering** Markdown correctly in the doc
preview and (b) a **source-format/prettify** action on the `.md` text.

**Recommended scope: interpretation (b) — a source formatter, i.e. make the
existing "Format Document" command work for `.md` files.** Interpretation (a) is
**already substantially done**: `web-ide/src/lib/markdown-live.js` is a mature
Obsidian-style CodeMirror 6 live-preview rendering GFM headings, emphasis, code,
blockquotes, links, rules, lists, task-list checkboxes, fenced code (with nested
syntax highlighting) and tables. The genuine gap is a prettify/canonicalise pass
on the Markdown source.

This mirrors the repo's philosophy: `crates/pseudoscript-format` provides
canonical, idempotent, WASM-safe formatting for `.pds`
(`crates/pseudoscript-format/src/lib.rs:1-13` — explicitly idempotent and
semantics-preserving), and CLAUDE.md treats canonical formatting as a
first-class norm. **The `.pds` Format command already exists end-to-end**
(confirmed): a "Format" button (`Toolbar.svelte:43`) and a `formatDocument`
keyboard command (`Editor.svelte:529`) both fire `onformat`, wired by the shell
to the wasm `format` import (`+page.svelte:4`, imported as `format as
formatSource`). So T12 = **"make Format also handle `.md`"** — extend a proven
flow, not build a new one. Today `onformat` only knows how to format `.pds`;
invoking it on a `.md` file is a no-op or routes the Markdown through the `.pds`
parser (which rejects it).

## Current state (file:line) — md lib + supported features/gaps

Renderer is **custom**, not `marked`/`markdown-it`. It is CodeMirror 6 + Lezer's
Markdown tree, driven by `@codemirror/lang-markdown` and `@lezer/markdown` (see
`web-ide/package.json` deps: `@codemirror/lang-markdown ^6.5.0`,
`@codemirror/language-data ^6.5.2`, `@lezer/markdown ^1.6.4`,
`@lezer/highlight ^1.2.3`). There is **no** `marked`, `markdown-it`, `remark`,
or `prettier` dependency anywhere in `web-ide`.

`web-ide/src/lib/markdown-live.js`:
- L520-538 `markdownLivePreview()` — the exported extension bundle.
- L524 GFM enabled via `markdown({ base, extensions: GFM, codeLanguages: languages })`; nested fenced-code languages lazy-load and are themed by `codeHighlightStyle` (L30-43).
- L64-70 inline marks rendered: StrongEmphasis, Emphasis, Strikethrough, InlineCode, Link.
- L105-129 task-list checkboxes (toggle rewrites source); L88-101 bullet glyphs; L268-322 headings/blockquotes/rules.
- L131-168 + L420-449 GFM tables rendered to real `<table>` via a `StateField` (block decorations); L207-208 table-cell inline regex.

Wiring: `Editor.svelte:42` `markdown` prop → `:543` `if (markdown) return
markdownLivePreview()`, swapped in a CM `Compartment` (`:541`, `:562-566`) when
the file type flips. Format command is renderer-agnostic — it goes through
`onformat` (`:529`), independent of `markdownLivePreview`.

**Rendering gaps (interpretation (a), if pursued — keep as a separate ticket):**
- **Autolinks / bare URLs** — only `[text](url)` styled; GFM bare-URL autolinking not handled.
- **Images** `![alt](src)` — no image widget; left as raw text. (Note: T4 already owns link/image resolution for the preview.)
- **Table cells** (L206-208) use a deliberate non-nesting inline subset — bold-inside-link etc. won't render in cells.
- **Footnotes / definition lists** — not handled (out of base GFM; acceptable to skip).

**Source-formatting gap (interpretation (b)):** none exists for `.md`. The
document stays verbatim by design (L3-4 comment); there is no prettify pass and
`onformat` has no `.md` branch.

## Proposed approach

Add a **client-side Markdown source formatter** and route the existing "Format
Document" affordance to it for `.md` files.

- **Engine:** `prettier/standalone` + `prettier/plugins/markdown` (browser ESM,
  no Node) — lowest-risk canonicalizer: consistent list markers, padded GFM
  table columns, heading normalisation, trailing-whitespace trim, fence
  normalisation. Alternative: `remark` + `remark-gfm` + `remark-stringify` (more
  configurable, larger graph). Prefer prettier for MVP: one opinionated call
  matching the "canonical formatting" ethos.
- **Wiring:** the **shell** (`+page.svelte`) already owns `onformat` and knows
  the active file's extension. Branch there: `.pds` → existing wasm
  `formatSource`; `.md` → new prettier wrapper. Editor/Toolbar are unchanged
  (the command already exists). Apply the result via the existing external-value
  path (`Editor.svelte:637-647`), which already replaces the doc in one
  transaction on reformat — undo stays one step and the cursor is remapped.
- Mirror the `.pds` formatter's contract: on a Markdown parse/format error,
  keep the original text and surface a toast (the shell already has `toast`
  state) — never silently mangle.
- Lazy-import the prettier chunk on first format so the initial bundle stays
  small (mirrors the lazy `codeLanguages` choice at `markdown-live.js:524`).

## Affected/new files

- `web-ide/package.json` — add `prettier` (uses `prettier/standalone` + `prettier/plugins/markdown`).
- `web-ide/src/routes/+page.svelte` — extension-dispatch in the `onformat` handler (`.pds` vs `.md`); error toast on failure.
- New `web-ide/src/lib/markdown-format.js` — thin async wrapper around `prettier.format(text, { parser: "markdown", plugins })`.
- `web-ide/src/lib/components/Editor.svelte` / `Toolbar.svelte` — no change expected (command + button already exist); confirm the Format button is enabled in `.md` mode.

## Open questions / decisions needed

- Prettier vs remark — lock the engine. (Recommend prettier.)
- Configurability: opinionated single config or expose `proseWrap`/`tabWidth`? MVP = opinionated, `proseWrap: "preserve"` (don't reflow author line breaks).
- Format-on-save, manual command, or both? MVP = manual (the existing command); format-on-save is a follow-up.
- Does prettier's table column re-padding round-trip cleanly through the GFM-table renderer? (Both are GFM; add a fixture to confirm.)
- Bundle size: prettier standalone + markdown plugin is non-trivial; confirm the lazy chunk is acceptable, else consider `remark` tree-shaken.
- Confirm where the shell decides `.md` vs `.pds` (it sets the `markdown` prop already) so the format dispatch reuses the same check rather than re-deriving it.

## Dependencies on other tasks (T4 doc preview, T10 new doc files)

- **T4 (doc preview links/images):** confirmed to share the same renderer
  (`markdown-live.js`) and to own relative link/image resolution and the
  folder-only-docs gating. T12 source-format is **independent** of T4's render
  work and can land in parallel; they touch different concerns (format vs
  render) and only co-locate in `markdown-live.js`/the shell.
- **T10 (new doc files):** not read this turn (no file present under
  `tasks/refinement/`); inferred to add `.md` docs. More docs increase the value
  of a formatter but it is not a blocker — T10 can supply real fixtures for T12
  acceptance tests. Confirm T10 scope before finalising this edge.

## Acceptance criteria (testable)

1. Invoking "Format" (button or shortcut) on a `.md` file reflows it to canonical Markdown (consistent list markers, padded GFM table columns, trimmed trailing whitespace) in a single undoable transaction.
2. Formatter is **idempotent**: formatting an already-formatted file produces no diff (mirrors `pseudoscript-format`).
3. Formatting a `.pds` file still routes to the wasm `.pds` formatter — no regression, no prettier applied to `.pds`.
4. On Markdown that fails to format, the original text is preserved and an error toast is shown (no silent corruption).
5. Live preview renders correctly immediately after a format (no decoration desync; cursor preserved/remapped).
6. The prettier chunk is lazy-loaded (absent from the initial editor bundle).
7. A fixture `.md` (headings, lists, a GFM table, a fenced code block) round-trips: format → render → format yields a stable result.

## Rough size (S/M/L) + parallel-safe?

**M.** The renderer exists and the Format command/plumbing exists, so this is
dependency wiring + one shell dispatch branch + a thin wrapper — not new parsing
or new UI. Risk concentrated in: bundle size/lazy-load and the table round-trip
fixture. **Parallel-safe** with T4/T10 — touches `package.json`, `+page.svelte`'s
`onformat`, and one new lib file; no overlap with the Rust crates or the
live-preview decoration logic. (If the interpretation-(a) rendering gaps are also
taken on, that sub-scope is S–M, touches `markdown-live.js` only, and should be a
separate ticket to preserve parallelism — and partly overlaps T4.)
