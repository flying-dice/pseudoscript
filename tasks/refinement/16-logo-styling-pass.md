# T16 — Logo + styling pass for go-live (#web-ide)

## Summary
The web-ide already ships a deliberate, coherent visual identity — the "Drafting
Terminal" theme (ink-black canvas, blueprint grid, single vermilion accent
`#ff5a36`, drafting corner ticks, mono micro-labels). It is *not* a from-scratch
job. The real go-live gaps are: (1) **no product logo/wordmark/favicon** — the
brand is a bare orange dot and the typeset word "PseudoScript"; the only `.svg`
logos in the tree belong to sample projects (`acme-tickets/logo.svg` etc.), not
the IDE; (2) **token gaps** — there is no spacing or shadow scale (paddings and
`box-shadow`s are ad-hoc literals throughout), and one dangling
`var(--shadow-lg)` reference resolves to nothing (a real bug); (3) a few **thin
empty/loading states**. Recommended scope: a real SVG logo + wordmark
(light/dark) + favicon, a consolidated token sheet (spacing + shadow + type
scales, fix the dangling shadow), and a prioritized component polish pass.
Dark-only is the current reality (light is design-deferred). Note the colour
system itself is already well-disciplined — even the CodeMirror editor theme is
authored entirely against the `--surface*`/`--ink*`/`--accent`/`--k-*` vars (0
hardcoded chrome hex), so the token work is *additive* (new scales), not a
cleanup of drift.

## Current state (file:line) — existing tokens/fonts/theme/logo audit

**Design system root — `web-ide/src/app.css`**
- Theme intent documented in the header comment (app.css:6-12): "Drafting
  Terminal".
- **Colour tokens** in `:root` (app.css:14-70): ink/surface ramp `--bg #0a0b0e`,
  `--surface`, `--surface-2`, `--surface-3` (app.css:15-19); text ramp `--ink`,
  `--ink-soft`, `--ink-faint` (20-22); `--line`, `--line-strong` (23-24); single
  accent `--accent #ff5a36` + `--accent-hi`, `--accent-ink`, `--accent-soft`
  (27-30); status `--ok`/`--warn`/`--err` (33-35); a light "drafting sheet"
  sub-palette `--sheet*`/`--tick` for diagrams (38-41); atmosphere `--grid` +
  `--glow` radial (44-45); C4 kind palette `--k-person/system/container/component/
  data/callable` (48-53); sequence return markers `--seq-ok/--seq-err` (56-57).
- **Type scale** (app.css:59-62): `--font-display` Bricolage Grotesque,
  `--font-sans` Hanken Grotesk, `--font-mono` JetBrains Mono. Loaded via Google
  Fonts `<link>` (app.html:8-13). Base 15px/1.6 (app.css:84-85).
- **Geometry tokens** (app.css:64-68): `--topbar-h 60px`, `--status-h 30px`,
  `--radius 10px`, `--radius-sm 7px`. **No spacing scale** — paddings/gaps are
  ad-hoc rem literals everywhere (`0.45rem`, `0.7rem`, `0.8rem`, `1.1rem`…).
- **No shadow tokens** — every `box-shadow` is an inline literal
  (page.svelte:1254, ProjectPanel:153, app.css:183/254…). One reference assumes a
  token that doesn't exist: `+page.svelte:1121 box-shadow: var(--shadow-lg)` →
  the md-help popover renders with **no shadow** (real bug, mechanical fix).
- `color-scheme: dark` set once (app.css:69); **no `prefers-color-scheme`, no
  `[data-theme]` light branch** anywhere in app.css. `app.html:2` pins
  `data-theme="dark"` statically.
- Nice existing touches to preserve: grain overlay (app.css:92-100), themed
  scrollbars (113-121), reduced-motion guard (356-362), Svelte-Flow var bridging
  (140-169).

**Theme / light-dark handling**
- `Settings.svelte` is **keyboard-shortcuts only** — no theme toggle (grep
  `theme` → 0 hits). The `[doc] theme` concept lives in the *built doc site*
  (`pds.toml [doc]`, parsed by `pseudoscript-doc/src/config.rs`), **not** the IDE
  chrome. So the IDE itself is dark-only today.

**Logo / favicon**
- `web-ide/static/favicon.svg` = `<circle r=9 fill=#ff5a36>` — a single orange
  dot, no glyph/wordmark.
- `Toolbar.svelte:14-17` brand = `.dot` (pulsing accent circle, 70-77) + typeset
  word "PseudoScript" in `--font-display` (78-83). Same pattern in
  `ProjectPanel.svelte:54-59,181-192`.
- **No product logo SVG exists.** The `logo.svg` files in the tree
  (`samples/acme-tickets/logo.svg`, `examples/*/logo.svg`,
  `pseudoscript/logo.svg`) are per-project doc-site logos, unrelated to the IDE.

**Editor theme — `Editor.svelte` (already clean)**
- The CodeMirror `EditorView.theme` (Editor.svelte:247-360) is authored entirely
  against CSS vars — `var(--ink)`, `--ink-faint`, `--surface`, `--surface-3`,
  `--line-strong`, `--accent`, `--accent-soft`, `--accent-hi`, `--radius-sm`, and
  the `--k-*` kind colours for completion icons. **0 hardcoded chrome hex.** So the
  editor already tracks the token sheet; no de-duplication needed here. (Only raw
  literals anywhere are the per-shadow `rgba(0,0,0,…)` and ad-hoc rem paddings,
  same as the rest of the app.)

**Empty / loading states**
- Loading curtain (page.svelte:971-974): mono "compiling the compiler…" + sweep
  bar — decent.
- `DiagramPane.svelte:14-22` empty = one muted `<p>{hint}</p>`, no icon/illo.
- `stage-empty` (page.svelte:1006-1012, 969) = bare grid backdrop behind the
  launcher — fine but plain.
- `ProblemsPane` / `FileTree` empty states are plain text lines.

## Proposed approach (design tokens, logo/favicon direction, component polish checklist)

**A. Token consolidation (mostly mechanical — `app.css`)**
1. Add a **spacing scale**: `--space-1…6` (e.g. 4/8/12/16/24/32px) and migrate the
   highest-traffic paddings/gaps (toolbar, content-bar, panes, launcher) — not a
   global sweep; convert chrome first.
2. Add a **shadow scale**: `--shadow-sm/-md/-lg/-modal`; define `--shadow-lg` (fixes
   the dangling ref at page.svelte:1121) and replace the inline literals in the
   modal/popover/dossier.
3. Add an explicit **type-size scale** (`--text-xs…xl`) so the dozens of
   `0.6–1.12rem` literals map to named steps. (The editor theme already uses vars,
   so no editor-colour rework is needed — just feed it the new scales.)

**B. Logo / wordmark / favicon (needs a brand decision, then SVG work)**
- Direction that fits the established language: a **monogram mark** built from the
  drafting-terminal vocabulary — e.g. a square "drafting frame" with corner ticks
  (already a motif in ProjectPanel `.tick`/`.ct`) enclosing a `>` prompt or a
  `ps`/`§` glyph, in `--accent` on transparent. Pairs with the existing
  `--font-display` wordmark.
- Deliver: `static/logo.svg` (mark), `static/wordmark.svg` (mark + "PseudoScript"
  lockup), `static/favicon.svg` (replace the bare dot with the monogram), plus a
  PNG/ICO fallback and apple-touch-icon. Light + dark variants (mark must read on
  both `--bg` and a light surface) even though the IDE is dark — the favicon and
  any future light mode / OG image need the light variant.
- Swap the typeset wordmark in `Toolbar.svelte:14-17` and `ProjectPanel.svelte:54-59`
  for the SVG lockup (keep the pulse animation on the mark's tick/dot).
- Add `<meta property="og:*">` + `theme-color` in `app.html` for shareable
  go-live links (currently only a `description`).

**C. Component polish checklist (prioritized)**
1. **Toolbar** (`Toolbar.svelte`) — adopt SVG logo lockup; align button heights to
   spacing scale; the glyph icons (`◳ ⚙ ⌨ ▾`) are font glyphs — consider unifying
   on a small inline-SVG icon set for crispness. *(High — first thing seen.)*
2. **Launcher / ProjectPanel** — already the most polished surface; mainly migrate
   to shadow/spacing tokens and the new logo. *(Med.)*
3. **Empty/loading states** — DiagramPane empty + `stage-empty`: add a subtle
   drafting-grid illustration or monogram watermark + a one-line CTA. *(Med —
   visible on every fresh load.)*
4. **Settings modal** — visually lighter than the launcher/build modal (smaller
   wordmark-less header); bring its header in line with the shared modal style.
   *(Low.)*
5. **Status bar / content-bar** — token migration only. *(Low.)*
6. **Shared modal primitive** — build/Settings/launcher each re-implement scrim +
   panel + ticks; extract one styled modal shell for consistency. *(Med, optional.)*

**Design decision vs mechanical**
- *Decision:* logo concept/monogram, wordmark lockup, whether to ship a light
  theme now or defer, icon-set direction (font glyph vs inline SVG).
- *Mechanical:* shadow/spacing/type tokens, fixing `--shadow-lg`, editor hex→var,
  meta tags, swapping the SVG in once it exists.

## Affected / new files
- New: `web-ide/static/logo.svg`, `web-ide/static/wordmark.svg`,
  `web-ide/static/apple-touch-icon.png` (+ optional `og-image.png`).
- Edit: `web-ide/static/favicon.svg` (replace dot with monogram).
- Edit: `web-ide/src/app.css` (spacing/shadow/type tokens; define `--shadow-lg`;
  optional light-theme scaffolding).
- Edit: `web-ide/src/app.html` (og/theme-color meta; favicon variants).
- Edit: `web-ide/src/lib/components/Toolbar.svelte`,
  `web-ide/src/lib/components/ProjectPanel.svelte` (SVG lockup).
- Edit: `web-ide/src/lib/components/DiagramPane.svelte`,
  `web-ide/src/routes/+page.svelte` (empty/loading states; `--shadow-lg`
  consumer at +page.svelte:1121).
- Optional new: a shared `Modal.svelte` shell.

## Open questions / decisions needed
- **Brand direction**: monogram concept (drafting frame + prompt? `§`? `ps`?) —
  needs a pick before SVG work. Use the `frontend-design` skill for this.
- **Name / wordmark**: confirm "PseudoScript" stays the displayed product name
  and casing; confirm GitHub link target (`flying-dice/pseudoscript`,
  Toolbar.svelte:14) is the canonical go-live URL.
- **Light vs dark default**: ship dark-only (current state) for go-live, or add a
  light theme + toggle in `Settings.svelte`? Recommend **dark-only now**, scaffold
  tokens so light is a follow-up. The `[doc] theme` (built sites) is a separate
  concern and out of scope.
- Icon set: keep Unicode font glyphs or move to inline SVG icons?

## Dependencies on other tasks
- **T17 (mobile/responsive)** — share the spacing/type token work; define
  responsive breakpoints in the same token pass so T16 and T17 don't fork the
  scale. The launcher already has one `@media (max-width:720px)` rule
  (ProjectPanel.svelte:223) to build on.
- Uses the **`frontend-design`** skill (do not invoke during refinement) for the
  logo concept and the polish execution.

## Acceptance criteria (testable + visual)
- A real PseudoScript logo/wordmark SVG exists and is shown in the Toolbar and the
  launcher in place of the typeset word; favicon is the monogram, not a bare dot.
- `app.html` carries favicon (svg + png/ico), apple-touch-icon, `theme-color`, and
  og: tags; tab + share-card preview look intentional.
- No dangling CSS-var references: `grep -r "var(--" web-ide/src` resolves every
  token (specifically `--shadow-lg` is defined and the md-help popover has its
  shadow back).
- Spacing + shadow + type token scales exist in `:root` and the app chrome
  (toolbar, content-bar, panes, modals) consume them.
- Empty/loading states (DiagramPane empty, `stage-empty`, curtain) show a branded
  watermark/illustration + CTA, not a lone muted line.
- Visual sign-off: toolbar, launcher, editor, canvas, empty states, and a built
  doc-site preview reviewed at desktop width and look launch-presentable and
  consistent. `npm run build` succeeds; no console errors.

## Rough size + parallel-safe?
- **M.** Mechanical token + meta + bug-fix work is small; the logo/wordmark design
  and empty-state illustrations are the variable cost (one brand decision gates
  them).
- **Parallel-safe with caveats.** The logo SVG work is independent. Token-sheet
  edits to `app.css` overlap with **T17** (responsive) — coordinate or sequence
  the shared token pass to avoid merge churn; everything else is parallelizable.
