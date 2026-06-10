// Builds the four committed bundles the Rust crate embeds:
//   ../src/assets/ssr.js      — SSR IIFE exposing globalThis.SSR.renderPage
//   ../src/assets/client.js   — progressive enhancement (no hydration)
//   ../src/assets/universe.js — the universe page's 3D island (three.js + d3-force-3d)
//   ../src/assets/style.css   — the site sheet (tokens + --pds-* palettes)
//
// Determinism: exact-pinned deps, ascii charset, no sourcemaps, no legal
// comments, fixed target. Run with `npm ci && npm run build`.

import { build } from "esbuild";
import sveltePlugin from "esbuild-svelte";
import { fileURLToPath } from "node:url";
import { existsSync, readFileSync, rmSync } from "node:fs";

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

// SSR: pure ECMAScript IIFE for QuickJS. Server-rendered markup only — no
// islands, no browser-only deps.
await build({
  ...shared,
  entryPoints: [fileURLToPath(new URL("./src/entry.server.js", import.meta.url))],
  outfile: out + "ssr.js",
  format: "iife",
  globalName: "SSR",
  platform: "neutral",
  mainFields: ["svelte", "module", "main"],
  conditions: ["svelte", "production"],
  plugins: [sveltePlugin({ compilerOptions: { generate: "server" }, emitCss: false })],
});

// Client: plain-JS progressive enhancement over the SSR markup. No Svelte in
// the bundle — entry.client.js imports only behaviors.js.
await build({
  ...shared,
  entryPoints: [fileURLToPath(new URL("./src/entry.client.js", import.meta.url))],
  outfile: out + "client.js",
  format: "iife",
  platform: "browser",
});

// Universe island: linked only by universe.html. Bundles three.js +
// d3-force-3d; never imported by the SSR components (guard below).
await build({
  ...shared,
  entryPoints: [fileURLToPath(new URL("./src/entry.universe.js", import.meta.url))],
  outfile: out + "universe.js",
  format: "iife",
  platform: "browser",
});

// CSS: the global sheet, standalone (no package imports).
await build({
  ...shared,
  minify: false,
  entryPoints: [fileURLToPath(new URL("./src/style.css", import.meta.url))],
  outfile: out + "style.css",
  loader: { ".css": "css" },
});

// The SSR build may extract component-scoped styles into an `ssr.css` sidecar;
// the SSR bundle runs in QuickJS (no DOM, no CSS), so drop it.
const ssrCss = out + "ssr.css";
if (existsSync(ssrCss)) rmSync(ssrCss);

// Guard: the SSR bundle must stay engine-free — a three.js leak (the universe
// island) would break QuickJS evaluation and balloon the embedded asset.
if (readFileSync(out + "ssr.js", "utf8").includes("WebGLRenderer")) {
  console.error("ssr.js contains WebGLRenderer — a browser-only island leaked into the SSR bundle");
  process.exit(1);
}

console.log("built ssr.js, client.js, universe.js, style.css → ../src/assets/");
