# PseudoScript Web IDE

An in-browser IDE for PseudoScript. The application runs entirely client-side as a
single WebAssembly module (`pseudoscript-ide`): live diagnostics, formatting,
completion, hover, and diagram preview with no backend. A static SvelteKit app,
deployable to Cloudflare Pages.

## Stack

- **SvelteKit** (Svelte 5), client-rendered (`ssr = false`), prerendered shell.
- **CodeMirror 6** editor with a PseudoScript language mode and a linter wired to
  the wasm's `check`.
- **`pseudoscript-ide`** — the IDE's single typed wasm (the stateful `IdeSession`),
  compiled with `wasm-pack`, vendored into `src/lib/pds-ide-wasm/` and
  **committed**, so deploys need no Rust toolchain. The TS facade is
  `src/lib/pds.ts`.

## Develop

```sh
cd web-ide
npm install
npm run dev
```

## Rebuild the wasm (only when the compiler changes)

Requires the Rust toolchain + [`wasm-pack`](https://rustwasm.github.io/wasm-pack/):

```sh
npm run build:wasm   # regenerates src/lib/pds-ide-wasm/ from ../crates/pseudoscript-ide
```

Commit the regenerated `src/lib/pds-ide-wasm/` so the hosted build stays
toolchain-free.

## Deploy to Cloudflare Pages

The app uses `@sveltejs/adapter-cloudflare`. In the Cloudflare Pages project:

| Setting | Value |
| --- | --- |
| Root directory | `web-ide` |
| Build command | `npm run build` |
| Build output directory | `.svelte-kit/cloudflare` |

No environment variables and no Rust are needed — the wasm is committed. The
result is a static, edge-served IDE.

(Direct upload also works: `npm run build` then
`npx wrangler pages deploy .svelte-kit/cloudflare`.)
