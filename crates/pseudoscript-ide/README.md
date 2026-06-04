# pseudoscript-ide

The PseudoScript web IDE as a single WebAssembly module. The whole browser API is
the stateful **`IdeSession`**: it holds the workspace (the consumer's `.pds`
modules plus the dependency externals resolved from `pds_modules/`, LANG.md §8.3)
and answers every query the IDE drives — language intelligence, diagram
projection, and the doc site — over that held state.

It consumes the toolchain (`pseudoscript-lsp-core`, `-model`, `-emit`, `-doc`,
`-format`) as ordinary Rust libraries, so there is **one** wasm in the browser and
no second bridge to drift from the app that uses it. The web IDE
(`web-ide/src/lib/pds.ts`) is the only client and drives `IdeSession` directly,
exercising the wasm toolchain as a real client.

## Typed boundary

The boundary is typed with [`tsify`](https://github.com/madonoharu/tsify): the Rust
DTOs (`Module`, `Completion`, `Hover`, `Diagnostic`, `References`, …) are the
source of truth, wasm-bindgen emits the real TypeScript interfaces, and values
cross as objects — no hand-written types, no `JSON.parse`. The one exception is the
render IR `Scene`, an opaque JSON string the canvas reads structurally.

## Build

```sh
# from web-ide/ — regenerates src/lib/pds-ide-wasm/ (committed)
npm run build:wasm

# or directly:
wasm-pack build crates/pseudoscript-ide --release --target web --out-dir <dir>
```

`crate-type = ["cdylib", "rlib"]`: the `cdylib` is the wasm artifact; the `rlib`
keeps the crate building and testing on the host (`cargo test -p pseudoscript-ide`).

## Surface

`IdeSession` is constructed once, then driven through two ports:

- **file system** — `mount(modules, externals)` (structural change), `set_source(fqn, text)` (per edit).
- **editor** — `diagnostics()`, `completion(fqn, offset)`, `hover(fqn, offset)`, `definition`, `references`, `rename_apply`, `outline`, `check`, `semantic_tokens`, `folding_ranges`, `format`, `emit_scene`/`symbol_scene`/`layout_scene` (Scene JSON), `dependency_modules`, `doc_manifest`, `render_doc_site`.

Language queries route through `pseudoscript-lsp-core` — the same handlers the
stdio `pseudoscript-lsp` server uses — so the browser and native editors share one
language core.
