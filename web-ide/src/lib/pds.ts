// The PseudoScript IDE wasm facade — one module over one wasm.
//
// `crates/pseudoscript-ide` compiles to a single `--target web` artifact whose
// whole API is the stateful `IdeSession`: it holds the workspace (the consumer's
// modules + the resolved dependency externals, LANG.md §8.3) and answers every
// query — language intelligence, diagrams, docs — over that held state. The
// boundary is typed by `tsify`, so values cross as objects (no `JSON.parse`); the
// one exception is the render IR `Scene`, an opaque JSON string the canvas reads
// structurally, which this module parses on the way out.
//
// JavaScript drives it through two ports: the file system pushes modules in
// (`mountIde`/`setIdeSource`), the editor pulls answers out (the `ide*` queries).
// Call `initWasm()` once before anything else.

import init, { IdeSession, version as wasmVersion } from "./pds-ide-wasm/pseudoscript_ide.js";
import type {
  Completion,
  Diagnostic,
  DocConfigInput,
  DocManifestGroup,
  FoldingRange,
  Hover,
  InitOutput,
  LocalInput,
  Module,
  ModuleResult,
  Occurrence,
  OutlineNode,
  References,
  RenamedSource,
  RenameSelection,
  RenderedFile,
  SemanticTokens,
  VendoredInput,
  DocManifest as WasmDocManifest,
} from "./pds-ide-wasm/pseudoscript_ide.js";
import { reportError } from "./errors.js";
import type { LayoutTweaks } from "./core/types.js";

// Re-export the generated DTOs so the rest of the app types against one source of
// truth — the Rust shapes — instead of hand-written interfaces that drift.
export type {
  Completion,
  Diagnostic,
  FoldingRange,
  Hover,
  Module,
  ModuleResult,
  Occurrence,
  OutlineNode,
  References,
  RenamedSource,
  RenameSelection,
  RenderedFile,
  SemanticTokens,
};
// Names the existing call sites use; aliased onto the generated types.
export type CompletionItem = Completion;
export type ModuleDiagnostics = ModuleResult;
export type DocManifest = WasmDocManifest;
export type DocConfig = DocConfigInput;
export type VendoredDepFile = VendoredInput;
export type LocalDepFile = LocalInput;
export type ManifestSection = DocManifestGroup;

/** A laid-out diagram scene or layout; structure is owned by the renderer. */
export type Scene = Record<string, unknown>;

// Run a wasm call, tagging any throw with its origin (PDS-WASM-001) so a
// language-server failure is never silent. Rethrows — callers that recover (the
// diagram canvas, formatting) still see the original error.
function callWasm<T>(name: string, fn: () => T): T {
  try {
    return fn();
  } catch (e) {
    reportError("WASM_CALL_FAILED", `${name}: ${String((e as Error)?.message ?? e)}`);
    throw e;
  }
}

let session: IdeSession | null = null;
let readyPromise: Promise<InitOutput> | undefined;

/** Initialise the wasm and create the session once; later calls reuse it. */
export function initWasm(): Promise<InitOutput> {
  if (!readyPromise) {
    readyPromise = init().then((out) => {
      session = new IdeSession();
      return out;
    });
  }
  return readyPromise;
}

/** The session after {@link initWasm}. Throws if used before init resolves. */
function ide(): IdeSession {
  if (!session) throw new Error("IdeSession not initialised — await initWasm() first");
  return session;
}

/** The compiler crate version. */
export function version(): string {
  return wasmVersion();
}

// ---- file-system port: push the workspace + edits in ----------------------

/** Load the workspace into the session: the consumer's modules and the resolved
 *  dependency externals. Called on a structural change, not per keystroke. */
export function mountIde(modules: Module[], externals: Module[] = []): void {
  ide().mount(modules, externals);
}

/** Apply an edit to one module's buffer (per keystroke). */
export function setIdeSource(fqn: string, text: string): void {
  ide().set_source(fqn, text);
}

// ---- editor port: queries over the held state -----------------------------

/** Workspace diagnostics, each module's problems attributed to it. */
export function ideDiagnostics(): ModuleResult[] {
  return ide().diagnostics();
}

/** Completions at byte `offset` in module `fqn`. */
export function ideCompletion(fqn: string, offset: number): Completion[] {
  return ide().completion(fqn, offset);
}

/** Markdown hover at byte `offset` in module `fqn`, or null. */
export function ideHover(fqn: string, offset: number): Hover | null {
  return ide().hover(fqn, offset) ?? null;
}

/** The declaration FQN for the symbol at `offset` in module `fqn`, or null. */
export function ideDefinition(fqn: string, offset: number): string | null {
  return callWasm("definition", () => ide().definition(fqn, offset) ?? null);
}

/** Find-usages for the symbol at `offset` in module `fqn`, or null. */
export function ideReferences(fqn: string, offset: number): References | null {
  return callWasm("references", () => ide().references(fqn, offset) ?? null);
}

/** Rename the symbol at `offset` in module `fqn` to `newName`, over the chosen
 *  occurrences. Returns the new full source of every module that changed. */
export function ideRename(
  fqn: string,
  offset: number,
  newName: string,
  selected: RenameSelection[],
): RenamedSource[] {
  return callWasm("rename_apply", () => ide().rename_apply(fqn, offset, newName, selected));
}

/** The nodes declared across the held workspace, for a diagram target picker. */
export function ideOutline(): OutlineNode[] {
  return ide().outline();
}

/** Project a diagram `view` over the held workspace to its laid-out scene. */
export function ideEmitScene(view: string, target = ""): Scene {
  return callWasm("emitScene", () => JSON.parse(ide().emit_scene(view, target)) as Scene);
}

/** The workspace as a software graph for the 3D relationship view: nodes (systems,
 *  containers, components, people) with containment, and directed relationships
 *  weighted by traffic. The renderer (`ForceGraph`) lays it out with d3-force-3d
 *  client-side. */
export type UniverseSnapshot = {
  nodes: { id: string; level: string; parent: string | null }[];
  edges: { from: string; to: string; traffic: number }[];
};

/** Build the software graph for the held workspace. */
export function ideUniverse(): UniverseSnapshot {
  return callWasm("universe", () => JSON.parse(ide().universe()) as UniverseSnapshot);
}

/** Project the fitting diagram for the symbol `fqn` to its scene. A throw here
 *  is control flow (a non-projectable symbol → the canvas lifeline fallback), so
 *  it is left unwrapped — `callWasm` would double-log it at error severity. */
export function ideSymbolScene(fqn: string): Scene {
  return JSON.parse(ide().symbol_scene(fqn)) as Scene;
}

/** Position a scene into absolute coordinates with the layout engine. `tweaks`
 * (optional) applies the C4 "Layout" toggles; ignored for other scene kinds. */
export function ideLayoutScene(scene: Scene, tweaks?: LayoutTweaks): Scene {
  const t = tweaks
    ? {
        minimize_long_edges: tweaks.minimizeLongEdges,
        orientation: tweaks.orientation,
        spacing: tweaks.spacing,
        experimental_grid: tweaks.experimentalGrid,
        grid_crossing_cost: tweaks.gridCrossingCost,
        grid_distance_cost: tweaks.gridDistanceCost,
        grid_flow_cost: tweaks.gridFlowCost,
        grid_search: tweaks.gridSearch,
        grid_pins: (tweaks.gridPins ?? []).map((p) => ({ fqn: p.fqn, row: p.row, col: p.col })),
      }
    : undefined;
  return callWasm(
    "layoutScene",
    () => JSON.parse(ide().layout_scene(JSON.stringify(scene), t)) as Scene,
  );
}

// ---- editor-local pure transforms (a single buffer, no held state) --------

/** Parse + static-check one `source` buffer (the per-keystroke lint path). */
export function check(source: string): Diagnostic[] {
  return ide().check(source);
}

/** Semantic tokens for one `source` buffer (delta-encoded over UTF-16). */
export function semanticTokens(source: string): SemanticTokens {
  return ide().semantic_tokens(source);
}

/** Foldable regions of one `source` buffer (0-based lines). */
export function foldRanges(source: string): FoldingRange[] {
  return ide().folding_ranges(source);
}

/** Format one `source` buffer to canonical form; throws on a parse error. */
export function format(source: string): string {
  return callWasm("format", () => ide().format(source));
}

// ---- project + docs --------------------------------------------------------

/**
 * Resolve the consumer workspace's direct dependencies (LANG.md §8.3) into
 * dependency-name-prefixed modules — the externals {@link mountIde} takes.
 * `lockToml` is the consumer's `pds.lock` (empty when absent); `vendored` are the
 * files read under `pds_modules/<slug>/`; `local` are local-source dependency
 * files (ADR-026).
 */
export function dependencyModules(
  lockToml: string,
  vendored: VendoredDepFile[],
  local: LocalDepFile[],
): Module[] {
  return callWasm("dependencyModules", () => ide().dependency_modules(lockToml, vendored, local));
}

/**
 * Parse a `pds.toml` string into its doc manifest (`{ name?, theme?, logo?,
 * sidebar: [{ title, items: [{ title, path }] }] }`). The caller loads each
 * page's `path` and folds the content into the config it hands {@link renderDocSite}.
 * Throws only on malformed TOML.
 */
export function docManifest(toml: string): DocManifest {
  return ide().doc_manifest(toml);
}

/** The `globalThis.SSR` the embedded Svelte bundle defines. */
interface SsrGlobal {
  renderPage: (propsJson: string) => string;
}

let ssrLoaded = false;

/**
 * Evaluate the embedded Svelte SSR bundle once, defining `globalThis.SSR`. The
 * browser is the JavaScript engine the site generator renders through (the
 * native CLI uses an embedded QuickJS instead).
 */
function ensureSsr(): void {
  if (ssrLoaded) return;
  // The bundle is an esbuild IIFE defining a top-level `var SSR`; that only
  // becomes the global `SSR` when run at global scope, so inject it as a
  // <script> (executes synchronously on append) rather than via `new Function`.
  const script = document.createElement("script");
  script.textContent = ide().doc_ssr_bundle();
  document.head.appendChild(script);
  script.remove();
  ssrLoaded = true;
}

/**
 * Render the whole documentation site for the held workspace — the browser
 * equivalent of the CLI's `pds doc`. `config` carries each authored page's
 * Markdown `content`. Returns `[{ path, contents }]`.
 */
export function renderDocSite(config: DocConfig): RenderedFile[] {
  ensureSsr();
  const ssr = (globalThis as unknown as { SSR: SsrGlobal }).SSR;
  const render = (propsJson: string): string => ssr.renderPage(propsJson);
  return callWasm("renderDocSite", () => ide().render_doc_site(config, render));
}
