# PseudoScript Web IDE

An in-browser IDE for PseudoScript. The compiler runs entirely client-side as
WebAssembly (`pseudoscript-wasm`): live diagnostics, formatting, and diagram
preview with no backend. A static SvelteKit app, deployable to Cloudflare Pages.

## Stack

- **SvelteKit** (Svelte 5), client-rendered (`ssr = false`), prerendered shell.
- **CodeMirror 6** editor with a PseudoScript language mode and a linter wired to
  the wasm compiler's `check`.
- **`pseudoscript-wasm`** — the compiler API compiled with `wasm-pack`, vendored
  into `src/lib/pds-wasm/` and **committed**, so deploys need no Rust toolchain.

## Develop

```sh
cd web-ide
npm install
npm run dev
```

## Rebuild the wasm (only when the compiler changes)

Requires the Rust toolchain + [`wasm-pack`](https://rustwasm.github.io/wasm-pack/):

```sh
npm run build:wasm   # regenerates src/lib/pds-wasm/ from ../crates/pseudoscript-wasm
```

Commit the regenerated `src/lib/pds-wasm/` so the hosted build stays
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
