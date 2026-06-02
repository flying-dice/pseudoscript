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
  rename_apply as wasmRenameApply,
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
import type { InitOutput } from "./pds-wasm/pseudoscript_wasm.js";
import { reportError } from "./errors.js";

// Run a wasm bridge call, tagging any throw with its origin (PDS-WASM-001) so a
// language-server failure is never silent. Rethrows — callers that recover (the
// editor's go-to-definition, the diagram canvas) still see the original error.
function callWasm<T>(name: string, fn: () => T): T {
  try {
    return fn();
  } catch (e) {
    reportError("WASM_CALL_FAILED", `${name}: ${String((e as Error)?.message ?? e)}`);
    throw e;
  }
}

// ---- Workspace + payload shapes ------------------------------------------

/** One workspace module: its fully-qualified name and source text. */
export interface Module {
  fqn: string;
  source: string;
}

/** A single diagnostic from the compiler (parse or static error). */
export interface Diagnostic {
  message: string;
  line: number;
  col: number;
  end_line: number;
  end_col: number;
  severity: string;
}

/** A module's diagnostics, attributed by {@link checkModules}. */
export interface ModuleDiagnostics {
  fqn: string;
  diagnostics: Diagnostic[];
}

/** One node declared in a module, as listed by {@link outline}. */
export interface OutlineNode {
  fqn: string;
  name: string;
  kind: string;
  triggered: boolean;
}

/** A laid-out diagram scene or layout; structure is owned by the renderer. */
export type Scene = Record<string, unknown>;

/** The symbol documentation carried on a {@link hover} result's `info`. */
export interface HoverInfo {
  title: string;
  body: string;
  svg?: string;
}

/** An LSP `Hover`, with the editor's `info` documentation payload. */
export interface Hover {
  contents: { kind: string; value: string };
  range?: unknown;
  info?: HoverInfo;
}

/** One occurrence of a symbol from {@link references} (1-based positions). */
export interface Occurrence {
  fqn: string;
  line: number;
  col: number;
  end_line: number;
  end_col: number;
  text: string;
  /** Char offsets into `text` bounding the symbol token, for highlighting. */
  match_start: number;
  match_end: number;
  decl: boolean;
}

/** The find-usages result from {@link references}. */
export interface References {
  fqn: string;
  title: string;
  occurrences: Occurrence[];
}

/** An LSP `CompletionItem` (`kind` is the integer `CompletionItemKind`). */
export interface CompletionItem {
  label: string;
  kind: number;
  detail?: string;
}

/** An LSP `SemanticTokens` payload (delta-encoded over UTF-16 positions). */
export interface SemanticTokens {
  data: number[];
  resultId?: string;
}

/** An LSP `FoldingRange` (0-based lines). */
export interface FoldingRange {
  startLine: number;
  endLine: number;
  kind?: string;
}

/** One page reference in a {@link docManifest} sidebar section. */
export interface ManifestItem {
  title: string;
  path: string;
}

/** One sidebar section of a {@link docManifest}. */
export interface ManifestSection {
  title: string;
  items: ManifestItem[];
}

/** The doc manifest parsed from a `pds.toml` by {@link docManifest}. */
export interface DocManifest {
  name?: string;
  theme?: string;
  logo?: string;
  lang?: string;
  sidebar: ManifestSection[];
}

/** One authored page handed to {@link renderDocSite}, with its Markdown content. */
export interface DocPageItem {
  title: string;
  path: string;
  content: string;
}

/** One sidebar section of a {@link renderDocSite} config. */
export interface DocConfigSection {
  title: string;
  items: DocPageItem[];
}

/** The site config handed to {@link renderDocSite}. */
export interface DocConfig {
  name: string;
  theme?: string;
  logo?: string;
  docs?: DocConfigSection[];
}

/** One rendered output file from {@link renderDocSite}. */
export interface RenderedFile {
  path: string;
  contents: string;
}

/** The `globalThis.SSR` the embedded Svelte bundle defines. */
interface SsrGlobal {
  renderPage: (propsJson: string) => string;
}

let readyPromise: Promise<InitOutput> | undefined;

/** Initialise the wasm module once; subsequent calls reuse the same promise. */
export function initWasm(): Promise<InitOutput> {
  if (!readyPromise) readyPromise = init();
  return readyPromise;
}

/** Parse + static-check one module; returns the diagnostics array. */
export function check(source: string): Diagnostic[] {
  return JSON.parse(wasmCheck(source));
}

/** Parse-only diagnostics (syntax errors), for fast per-keystroke feedback. */
export function parse(source: string): Diagnostic[] {
  return JSON.parse(wasmParse(source));
}

/**
 * Check a whole workspace. `modules` is `[{ fqn, source }]`; returns
 * `[{ fqn, diagnostics }]` with cross-module errors attributed per module.
 */
export function checkModules(modules: Module[]): ModuleDiagnostics[] {
  return JSON.parse(wasmCheckModules(JSON.stringify(modules)));
}

/** Format source to canonical form; throws on a parse error. */
export function format(source: string): string {
  return wasmFormat(source);
}

/** Project a diagram view to its laid-out scene object. */
export function emitScene(source: string, view: string, target = ""): Scene {
  return callWasm("emitScene", () => JSON.parse(wasmEmitScene(source, view, target)));
}

/**
 * List the nodes declared in `source`: `[{ fqn, name, kind, triggered }]`.
 * Used to derive a diagram view's target options from the model itself.
 */
export function outline(source: string): OutlineNode[] {
  return JSON.parse(wasmOutline(source));
}

/** Like {@link outline}, but over a whole workspace (`[{ fqn, source }]`). */
export function outlineModules(modules: Module[]): OutlineNode[] {
  return JSON.parse(wasmOutlineModules(JSON.stringify(modules)));
}

/**
 * Project a diagram view over the whole workspace, so it shows nodes and edges
 * across modules (a container's components, cross-system calls).
 */
export function emitSceneModules(modules: Module[], view: string, target = ""): Scene {
  return callWasm("emitSceneModules", () => JSON.parse(wasmEmitSceneModules(JSON.stringify(modules), view, target)));
}

/**
 * Position a (sequence) scene into absolute coordinates with the layout engine.
 * `scene` is a sequence Scene object (optionally depth-collapsed first); returns
 * the positioned `Layout` the renderer draws verbatim.
 */
export function layoutScene(scene: Scene): Scene {
  return callWasm("layoutScene", () => JSON.parse(wasmLayoutScene(JSON.stringify(scene))));
}

/** Project a diagram view to an SVG string. */
export function emitSvg(source: string, view: string, target = ""): string {
  return callWasm("emitSvg", () => wasmEmitSvg(source, view, target));
}

/**
 * Resolve the symbol under a byte `offset` in module `moduleFqn` as a standard
 * LSP `Hover` (`{ contents: { kind, value }, range }`, Markdown), or `null` when
 * the cursor rests on no symbol. Served exactly as the stdio LSP serves it — no
 * diagram. `modules` is `[{ fqn, source }]`.
 */
export function hover(modules: Module[], moduleFqn: string, offset: number): Hover | null {
  return callWasm("hover", () => JSON.parse(wasmHover(JSON.stringify(modules), moduleFqn, offset)));
}

/**
 * Resolve the symbol under a byte `offset` in module `moduleFqn` to the FQN of
 * its declaration (go-to-definition), or `null` when the cursor rests on no
 * resolvable symbol. Cheaper than {@link hover} — no diagram. `modules` is
 * `[{ fqn, source }]`.
 */
export function definition(modules: Module[], moduleFqn: string, offset: number): string | null {
  return callWasm("definition", () => JSON.parse(wasmDefinition(JSON.stringify(modules), moduleFqn, offset)));
}

/**
 * Find every usage of the symbol under a byte `offset` in module `moduleFqn`
 * across the workspace. Returns `{ fqn, title, occurrences: [{ fqn, line, col,
 * end_line, end_col, text, decl }] }` (1-based positions, a trimmed source-line
 * preview, `decl` marking the declaration), or `null` when the cursor rests on
 * no resolvable symbol. `modules` is `[{ fqn, source }]`.
 */
export function references(modules: Module[], moduleFqn: string, offset: number): References | null {
  return callWasm("references", () => JSON.parse(wasmReferences(JSON.stringify(modules), moduleFqn, offset)));
}

/** An occurrence the user chose to rename, keyed as {@link references} reports it. */
export interface RenameSelection {
  fqn: string;
  line: number;
  col: number;
}

/** One module's rewritten source after a rename ({@link renameApply}). */
export interface RenamedSource {
  fqn: string;
  source: string;
}

/**
 * Renames the symbol under `offset` in `moduleFqn` to `newName`, applying only
 * the `selected` occurrences (as {@link references} reports them). Returns the
 * new full source of every module that changed — the host swaps these into its
 * buffers. Throws when `newName` is not a valid identifier.
 */
export function renameApply(
  modules: Module[],
  moduleFqn: string,
  offset: number,
  newName: string,
  selected: RenameSelection[],
): RenamedSource[] {
  return callWasm("rename_apply", () =>
    JSON.parse(wasmRenameApply(JSON.stringify(modules), moduleFqn, offset, newName, JSON.stringify(selected))),
  );
}

/**
 * Context-aware completion at a byte `offset` in module `moduleFqn`, as standard
 * LSP `CompletionItem`s (`[{ label, kind, detail }]`, where `kind` is the
 * integer `CompletionItemKind`). Scoped to the trigger before the caret
 * (`.`/`::`/`#[`/type-position/general); the editor filters against the typed
 * prefix. Served exactly as the stdio LSP serves it. `modules` is
 * `[{ fqn, source }]`.
 */
export function completion(modules: Module[], moduleFqn: string, offset: number): CompletionItem[] {
  return callWasm("completion", () => JSON.parse(wasmCompletion(JSON.stringify(modules), moduleFqn, offset)));
}

/**
 * AST-aware semantic tokens for `source` as a standard LSP `SemanticTokens`
 * (`{ data: [Δline, Δstart, len, type, mods], … }`, delta-encoded over UTF-16
 * positions). `type` indexes the legend in `pseudoscript-lsp-core::semantic`.
 * The same colouring the stdio LSP serves — no hand-written tokenizer.
 */
export function semanticTokens(source: string): SemanticTokens {
  return JSON.parse(wasmSemanticTokens(source));
}

/**
 * Foldable regions of `source` as standard LSP `FoldingRange`s
 * (`[{ startLine, endLine, kind }]`, 0-based lines) — every multi-line
 * declaration and statement block, from the same AST-accurate fold logic the
 * LSP serves.
 */
export function foldRanges(source: string): FoldingRange[] {
  return JSON.parse(wasmFoldingRanges(source));
}

/**
 * Project the fitting diagram for a symbol to its laid-out scene object (the
 * interactive counterpart of {@link hover}'s `svg`, for a side panel or
 * full-screen view). `modules` is `[{ fqn, source }]`.
 */
// A throw here is control flow, not an error: a non-projectable symbol (a
// feature, a data type) signals the canvas to show its lifeline fallback, which
// reports PDS-DIAGRAM-001. Wrapping it in `callWasm` would double-log at error
// severity — leave it to the caller's recovery.
export function symbolScene(modules: Module[], fqn: string): Scene {
  return JSON.parse(wasmSymbolScene(JSON.stringify(modules), fqn));
}

/**
 * Render the fitting diagram for a symbol to a self-contained SVG string (the
 * live, re-derivable form of {@link hover}'s `svg`, for a docked side panel).
 * `modules` is `[{ fqn, source }]`.
 */
export function symbolSvg(modules: Module[], fqn: string): string {
  return wasmSymbolSvg(JSON.stringify(modules), fqn);
}

/** The compiler crate version. */
export function version(): string {
  return wasmVersion();
}

let ssrLoaded = false;

/**
 * Evaluate the embedded Svelte SSR bundle once, defining `globalThis.SSR`. The
 * browser is the JavaScript engine the compiler's site generator renders
 * through (the native CLI uses an embedded QuickJS instead).
 */
function ensureSsr(): void {
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
export function docManifest(toml: string): DocManifest {
  return JSON.parse(wasmDocManifest(toml));
}

/**
 * Render the whole documentation site for a workspace — the browser equivalent
 * of the CLI's `pds doc`. `modules` is `[{ fqn, source }]`; `config` is
 * `{ name, theme?, logo?, docs?: [{ title, items: [{ title, path, content }] }] }`
 * — `docs` carries each authored page's Markdown `content`. Returns
 * `[{ path, contents }]`.
 */
export function renderDocSite(modules: Module[], config: DocConfig): RenderedFile[] {
  ensureSsr();
  const ssr = (globalThis as unknown as { SSR: SsrGlobal }).SSR;
  const render = (propsJson: string): string => ssr.renderPage(propsJson);
  return JSON.parse(wasmRenderDocSite(JSON.stringify(modules), JSON.stringify(config), render));
}
