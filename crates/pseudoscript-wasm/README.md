# pseudoscript-wasm

The PseudoScript compiler API as a WebAssembly module, for JavaScript hosts —
browser, Bun, Node, Deno. A thin, transport-free façade over the language core
(`pseudoscript-syntax`, `-format`, `-model`, `-emit`). No JS toolchain or server
is needed at runtime: ship the `.wasm` + generated glue from a static host.

## Build

Requires the wasm target and `wasm-bindgen` glue. The easy path is
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack            # once

# Bundler (Vite/esbuild), Bun, browsers:
wasm-pack build crates/pseudoscript-wasm --release --target bundler
# Node CommonJS:    --target nodejs
# <script> / Deno:  --target web
```

This emits a `pkg/` with `pseudoscript_wasm_bg.wasm`, the JS bindings, `.d.ts`
types, and a `package.json`.

Without `wasm-pack`:

```sh
cargo build -p pseudoscript-wasm --release --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/pseudoscript_wasm.wasm \
  --out-dir pkg --target bundler
```

## API

Every function is JSON in / JSON out (or plain strings); identical across hosts.

| Function | Returns | Use |
| --- | --- | --- |
| `version()` | `string` | crate version |
| `parse(source)` | `Diagnostic[]` JSON | syntax errors only (fast, per-keystroke) |
| `check(source)` | `Diagnostic[]` JSON | parse + static analysis (one module) |
| `check_modules(json)` | `{fqn, diagnostics}[]` JSON | multi-module workspace |
| `format(source)` | `string` | canonical formatting (throws on parse error) |
| `emit_scene(source, view, target)` | `Scene` JSON | diagram geometry |
| `emit_svg(source, view, target)` | `string` | rendered SVG |

`view` ∈ `context` \| `container` \| `component` \| `sequence`; `target` is the
boundary/entry FQN (ignored for `context`). `check_modules` input is
`[{ "fqn": string, "source": string }]`.

A `Diagnostic` is:

```ts
{ severity: "error" | "warning" | "info"; message: string; code: string | null;
  start: number; end: number;            // byte offsets
  start_line: number; start_col: number; // 1-based
  end_line: number; end_col: number; }
```

## Usage (Bun / Node)

```js
import init, { check, format, emit_svg } from "./pkg/pseudoscript_wasm.js";
await init?.(); // web/bundler targets initialise the wasm; nodejs is sync

const src = "//! shop\npublic person Customer;\npublic system Shop;";
const diagnostics = JSON.parse(check(src));   // []  → well-formed
const pretty = format(src);                   // canonical text
const svg = emit_svg(src, "context", "");     // <svg ...>
```

## Toward LSP-over-wasm

The diagnostics here are the substrate for an in-browser language server. The
analysis logic lives in `pseudoscript-model` (wasm-clean); only the
`pseudoscript-lsp` transport (`tower-lsp` + `tokio`) is native-only. For a web
IDE, add request-shaped functions to this crate — `hover(src, offset)`,
`completions(src, offset)`, `definition(src, offset)` — returning LSP-shaped
JSON, and drive them from the editor (Monaco/CodeMirror) or a JS-side
`monaco-languageclient` loop. No `tokio`, no socket: the editor calls wasm
directly.
