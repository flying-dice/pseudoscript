// Builds the three committed bundles the Rust crate embeds:
//   ../src/assets/ssr.js     — SSR IIFE exposing globalThis.SSR.renderPage
//   ../src/assets/client.js  — browser hydration + diagram islands
//   ../src/assets/style.css  — global sheet + Svelte Flow + timeline styles
//
// Determinism: exact-pinned deps, ascii charset, no sourcemaps, no legal
// comments, fixed target. Run with `npm ci && npm run build`.

import { build } from "esbuild";
import sveltePlugin from "esbuild-svelte";
import { fileURLToPath } from "node:url";
import { appendFileSync, existsSync, readFileSync, rmSync } from "node:fs";

const out = fileURLToPath(new URL("../src/assets/", import.meta.url));

const shared = {
  bundle: true,
  minify: true,
  charset: "ascii",
  legalComments: "none",
  sourcemap: false,
  target: ["es2020"],
  define: { "process.env.NODE_ENV": '"production"' },
};

// SSR: pure ECMAScript IIFE for QuickJS. The diagram islands (Svelte Flow,
// dagre) are dynamically imported only in the browser, so they are marked
// external here and never enter the SSR bundle.
await build({
  ...shared,
  entryPoints: [fileURLToPath(new URL("./src/entry.server.js", import.meta.url))],
  outfile: out + "ssr.js",
  format: "iife",
  globalName: "SSR",
  platform: "neutral",
  mainFields: ["svelte", "module", "main"],
  conditions: ["svelte", "production"],
  external: ["@xyflow/svelte", "@dagrejs/dagre"],
  plugins: [sveltePlugin({ compilerOptions: { generate: "server" }, emitCss: false })],
});

// Client: browser bundle, hydrates the SSR markup and mounts the diagram
// islands. Svelte components compile in DOM (client) mode.
await build({
  ...shared,
  entryPoints: [fileURLToPath(new URL("./src/entry.client.js", import.meta.url))],
  outfile: out + "client.js",
  format: "iife",
  platform: "browser",
  mainFields: ["svelte", "browser", "module", "main"],
  conditions: ["svelte", "browser", "production"],
  plugins: [sveltePlugin({ compilerOptions: { generate: "client" }, emitCss: false })],
});

// CSS: the global sheet @imports the Svelte Flow stylesheet; esbuild resolves
// the bare package import via node resolution.
await build({
  ...shared,
  minify: false,
  entryPoints: [fileURLToPath(new URL("./src/style.css", import.meta.url))],
  outfile: out + "style.css",
  loader: { ".css": "css" },
});

// The client build extracts Svelte Flow's component-scoped styles into a
// `client.css` sidecar. The page loads only `style.css`, so fold those rules
// in and drop the sidecar (deterministic: same inputs → same output).
const clientCss = out + "client.css";
if (existsSync(clientCss)) {
  appendFileSync(out + "style.css", "\n" + readFileSync(clientCss, "utf8"));
  rmSync(clientCss);
}

console.log("built ssr.js, client.js, style.css → ../src/assets/");
