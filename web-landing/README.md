# web-landing

PseudoScript marketing landing page. Plain **Svelte 5 + Vite** — self-contained, no SvelteKit, shares nothing with `web-ide`.

Ported from the Claude Design handoff (`Landing Page.html`): the "drafting terminal" aesthetic — ink canvas, vermilion horizon, hairline frames, mono micro-labels.

## Run

```bash
npm install
npm run dev      # dev server
npm run build    # production build → dist/
npm run preview  # serve the build
```

## Layout

| Path | Role |
| --- | --- |
| `src/styles/colors_and_type.css` | design tokens (dark + light), type scale, motion — verbatim from the kit |
| `src/styles/landing.css` | page styles built on the tokens — verbatim from the kit |
| `src/lib/pds-syntax.js` | `.pds` tokenizer + highlighter |
| `src/lib/landing-anim.js` | scroll-driven engine: hero typing, C4 assembly, diagnostics resolve, sequence diagram, reveals |
| `src/components/*.svelte` | one component per section (Topbar, Hero, Convergence, Workflow, IdeShowcase, Packages, Cta, SiteFooter) |
| `src/components/Tweaks.svelte` | theme / accent / motion panel (Svelte, replaces the original React island) |

Section CSS uses global class names (the two stylesheets are imported globally in `main.js`), so component markup keeps the kit's classes intact; `landing-anim.js` queries the mounted DOM by id and runs once from `App.svelte`'s `onMount`.

Icons come from `@lucide/svelte` (replacing the original's lucide CDN). The CLI and JetBrains-extension cards link to `#` placeholders — wire real URLs when available.
