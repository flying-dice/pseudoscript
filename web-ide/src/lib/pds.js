// Loader + typed wrappers around the PseudoScript compiler wasm module.
//
// The vendored `pds-wasm/` package is wasm-bindgen's `--target web` output:
// the default export initialises the module (fetching the `.wasm`), after which
// the named functions are synchronous. Call `initWasm()` once before using them.
import init, {
  check as wasmCheck,
  check_modules as wasmCheckModules,
  parse as wasmParse,
  format as wasmFormat,
  emit_scene as wasmEmitScene,
  emit_scene_modules as wasmEmitSceneModules,
  layout_scene as wasmLayoutScene,
  emit_svg as wasmEmitSvg,
  hover as wasmHover,
  definition as wasmDefinition,
  references as wasmReferences,
  completion as wasmCompletion,
  semantic_tokens as wasmSemanticTokens,
  folding_ranges as wasmFoldingRanges,
  doc_ssr_bundle as wasmDocSsrBundle,
  doc_manifest as wasmDocManifest,
  outline as wasmOutline,
  outline_modules as wasmOutlineModules,
  render_doc_site as wasmRenderDocSite,
  symbol_scene as wasmSymbolScene,
  symbol_svg as wasmSymbolSvg,
  version as wasmVersion,
} from "./pds-wasm/pseudoscript_wasm.js";

let readyPromise;

/** Initialise the wasm module once; subsequent calls reuse the same promise. */
export function initWasm() {
  if (!readyPromise) readyPromise = init();
  return readyPromise;
}

/** Parse + static-check one module; returns the diagnostics array. */
export function check(source) {
  return JSON.parse(wasmCheck(source));
}

/** Parse-only diagnostics (syntax errors), for fast per-keystroke feedback. */
export function parse(source) {
  return JSON.parse(wasmParse(source));
}

/**
 * Check a whole workspace. `modules` is `[{ fqn, source }]`; returns
 * `[{ fqn, diagnostics }]` with cross-module errors attributed per module.
 */
export function checkModules(modules) {
  return JSON.parse(wasmCheckModules(JSON.stringify(modules)));
}

/** Format source to canonical form; throws on a parse error. */
export function format(source) {
  return wasmFormat(source);
}

/** Project a diagram view to its laid-out scene object. */
export function emitScene(source, view, target = "") {
  return JSON.parse(wasmEmitScene(source, view, target));
}

/**
 * List the nodes declared in `source`: `[{ fqn, name, kind, triggered }]`.
 * Used to derive a diagram view's target options from the model itself.
 */
export function outline(source) {
  return JSON.parse(wasmOutline(source));
}

/** Like {@link outline}, but over a whole workspace (`[{ fqn, source }]`). */
export function outlineModules(modules) {
  return JSON.parse(wasmOutlineModules(JSON.stringify(modules)));
}

/**
 * Project a diagram view over the whole workspace, so it shows nodes and edges
 * across modules (a container's components, cross-system calls).
 */
export function emitSceneModules(modules, view, target = "") {
  return JSON.parse(wasmEmitSceneModules(JSON.stringify(modules), view, target));
}

/**
 * Position a (sequence) scene into absolute coordinates with the layout engine.
 * `scene` is a sequence Scene object (optionally depth-collapsed first); returns
 * the positioned `Layout` the renderer draws verbatim.
 */
export function layoutScene(scene) {
  return JSON.parse(wasmLayoutScene(JSON.stringify(scene)));
}

/** Project a diagram view to an SVG string. */
export function emitSvg(source, view, target = "") {
  return wasmEmitSvg(source, view, target);
}

/**
 * Resolve the symbol under a byte `offset` in module `moduleFqn` as a standard
 * LSP `Hover` (`{ contents: { kind, value }, range }`, Markdown), or `null` when
 * the cursor rests on no symbol. Served exactly as the stdio LSP serves it — no
 * diagram. `modules` is `[{ fqn, source }]`.
 */
export function hover(modules, moduleFqn, offset) {
  return JSON.parse(wasmHover(JSON.stringify(modules), moduleFqn, offset));
}

/**
 * Resolve the symbol under a byte `offset` in module `moduleFqn` to the FQN of
 * its declaration (go-to-definition), or `null` when the cursor rests on no
 * resolvable symbol. Cheaper than {@link hover} — no diagram. `modules` is
 * `[{ fqn, source }]`.
 */
export function definition(modules, moduleFqn, offset) {
  return JSON.parse(wasmDefinition(JSON.stringify(modules), moduleFqn, offset));
}

/**
 * Find every usage of the symbol under a byte `offset` in module `moduleFqn`
 * across the workspace. Returns `{ fqn, title, occurrences: [{ fqn, line, col,
 * end_line, end_col, text, decl }] }` (1-based positions, a trimmed source-line
 * preview, `decl` marking the declaration), or `null` when the cursor rests on
 * no resolvable symbol. `modules` is `[{ fqn, source }]`.
 */
export function references(modules, moduleFqn, offset) {
  return JSON.parse(wasmReferences(JSON.stringify(modules), moduleFqn, offset));
}

/**
 * Context-aware completion at a byte `offset` in module `moduleFqn`, as standard
 * LSP `CompletionItem`s (`[{ label, kind, detail }]`, where `kind` is the
 * integer `CompletionItemKind`). Scoped to the trigger before the caret
 * (`.`/`::`/`#[`/type-position/general); the editor filters against the typed
 * prefix. Served exactly as the stdio LSP serves it. `modules` is
 * `[{ fqn, source }]`.
 */
export function completion(modules, moduleFqn, offset) {
  return JSON.parse(wasmCompletion(JSON.stringify(modules), moduleFqn, offset));
}

/**
 * AST-aware semantic tokens for `source` as a standard LSP `SemanticTokens`
 * (`{ data: [Δline, Δstart, len, type, mods], … }`, delta-encoded over UTF-16
 * positions). `type` indexes the legend in `pseudoscript-lsp-core::semantic`.
 * The same colouring the stdio LSP serves — no hand-written tokenizer.
 */
export function semanticTokens(source) {
  return JSON.parse(wasmSemanticTokens(source));
}

/**
 * Foldable regions of `source` as standard LSP `FoldingRange`s
 * (`[{ startLine, endLine, kind }]`, 0-based lines) — every multi-line
 * declaration and statement block, from the same AST-accurate fold logic the
 * LSP serves.
 */
export function foldRanges(source) {
  return JSON.parse(wasmFoldingRanges(source));
}

/**
 * Project the fitting diagram for a symbol to its laid-out scene object (the
 * interactive counterpart of {@link hover}'s `svg`, for a side panel or
 * full-screen view). `modules` is `[{ fqn, source }]`.
 */
export function symbolScene(modules, fqn) {
  return JSON.parse(wasmSymbolScene(JSON.stringify(modules), fqn));
}

/**
 * Render the fitting diagram for a symbol to a self-contained SVG string (the
 * live, re-derivable form of {@link hover}'s `svg`, for a docked side panel).
 * `modules` is `[{ fqn, source }]`.
 */
export function symbolSvg(modules, fqn) {
  return wasmSymbolSvg(JSON.stringify(modules), fqn);
}

/** The compiler crate version. */
export function version() {
  return wasmVersion();
}

let ssrLoaded = false;

/**
 * Evaluate the embedded Svelte SSR bundle once, defining `globalThis.SSR`. The
 * browser is the JavaScript engine the compiler's site generator renders
 * through (the native CLI uses an embedded QuickJS instead).
 */
function ensureSsr() {
  if (ssrLoaded) return;
  // The bundle is an esbuild IIFE that defines a top-level `var SSR`; that only
  // becomes the global `SSR` when run at global scope, so inject it as a
  // <script> (executes synchronously on append) rather than via `new Function`.
  const script = document.createElement("script");
  script.textContent = wasmDocSsrBundle();
  document.head.appendChild(script);
  script.remove();
  ssrLoaded = true;
}

/**
 * Parse a `pds.toml` string into its doc manifest:
 * `{ name?, theme?, logo?, sidebar: [{ title, items: [{ title, path }] }] }`.
 * The caller loads each page's `path` and folds the content into the config it
 * hands {@link renderDocSite}. Uses the same TOML parser as the CLI. Returns an
 * empty manifest (`{ sidebar: [] }`) for a `pds.toml` with no `[doc]` table;
 * throws only on malformed TOML.
 */
export function docManifest(toml) {
  return JSON.parse(wasmDocManifest(toml));
}

/**
 * Render the whole documentation site for a workspace — the browser equivalent
 * of the CLI's `pds doc`. `modules` is `[{ fqn, source }]`; `config` is
 * `{ name, theme?, logo?, docs?: [{ title, items: [{ title, path, content }] }] }`
 * — `docs` carries each authored page's Markdown `content`. Returns
 * `[{ path, contents }]`.
 */
export function renderDocSite(modules, config) {
  ensureSsr();
  const render = (propsJson) => globalThis.SSR.renderPage(propsJson);
  return JSON.parse(wasmRenderDocSite(JSON.stringify(modules), JSON.stringify(config), render));
}
