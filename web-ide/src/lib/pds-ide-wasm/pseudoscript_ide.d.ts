/* tslint:disable */
/* eslint-disable */
/**
 * A foldable region, 0-based lines — the editor folds these instead of
 * brace-matching in JS — tagged with the kind of construct it covers.
 */
export interface FoldingRange {
    startLine: number;
    endLine: number;
    kind: FoldKind;
}

/**
 * A node pinned to a grid cell, by FQN. Crosses the wasm boundary as an object.
 */
export interface GridPin {
    fqn: string;
    row: number;
    col: number;
}

/**
 * AST-aware semantic tokens: the delta-encoded flat `data` array over UTF-16
 * positions, identical to the stdio server\'s `semanticTokens/full` response.
 */
export interface SemanticTokens {
    data: number[];
}

/**
 * Markdown hover content — the editor reads `contents.value`.
 */
export interface Hover {
    contents: MarkupContent;
}

/**
 * One `(fqn, source)` module — the file-system port\'s unit. Input to `mount`
 * and `set_source`, output of [`IdeSession::dependency_modules`].
 */
export interface Module {
    fqn: string;
    source: string;
}

/**
 * One `[[doc.sidebar]]` group in the manifest. `title` is required (a group is
 * a named heading); `items` defaults to empty.
 */
export interface DocManifestGroup {
    title: string;
    items?: DocManifestItem[];
}

/**
 * One `{ title, path }` page entry in the manifest — both required.
 */
export interface DocManifestItem {
    title: string;
    path: string;
}

/**
 * One completion item: the label, the integer LSP `CompletionItemKind` (the
 * editor maps it to an icon), and an optional detail.
 */
export interface Completion {
    label: string;
    kind: number;
    detail: string | undefined;
}

/**
 * One declared node (or `feature`), for the outline / diagram target picker.
 * `line`/`col` are the 1-based position of the name in its own module;
 * `parent` is the FQN of the enclosing node (the C4 containment, §6) or `null`.
 */
export interface OutlineNode {
    fqn: string;
    name: string;
    kind: string;
    triggered: boolean;
    line: number;
    col: number;
    parent: string | undefined;
    summary: string | undefined;
}

/**
 * One diagnostic: its byte span plus 1-based line/column endpoints, so the
 * editor can place a squiggle without re-indexing the source.
 */
export interface Diagnostic {
    severity: string;
    message: string;
    code: string | undefined;
    start: number;
    end: number;
    start_line: number;
    start_col: number;
    end_line: number;
    end_col: number;
}

/**
 * One directed relationship in the 3D graph, weighted by traffic (call count).
 */
export interface UniverseEdge {
    from: string;
    to: string;
    traffic: number;
}

/**
 * One doc group in [`DocConfigInput`]: a heading and its pages (with content).
 */
export interface DocGroupInput {
    title?: string;
    items?: DocItemInput[];
}

/**
 * One find-usages hit: its module `fqn`, 1-based span, the trimmed source line
 * for a preview, and `decl` marking the declaration site. `match_start`/
 * `match_end` are char offsets into `text` bounding the symbol token.
 */
export interface Occurrence {
    fqn: string;
    line: number;
    col: number;
    end_line: number;
    end_col: number;
    text: string;
    match_start: number;
    match_end: number;
    decl: boolean;
}

/**
 * One local-source dependency file for `dependency_modules`: the dependency
 * name (ADR-026), its FQN within the dependency workspace, and its source.
 */
export interface LocalInput {
    name: string;
    fqn: string;
    source: string;
}

/**
 * One module\'s diagnostics in the workspace check result.
 */
export interface ModuleResult {
    fqn: string;
    diagnostics: Diagnostic[];
}

/**
 * One module\'s rewritten source after a rename — output of `rename_apply`.
 */
export interface RenamedSource {
    fqn: string;
    source: string;
}

/**
 * One node in the 3D relationship graph: its FQN, C4 level, and containment parent.
 */
export interface UniverseNode {
    id: string;
    level: string;
    parent: string | undefined;
}

/**
 * One occurrence the host chose to rename, keyed by module `fqn` and the
 * 1-based `line`/`col` [`References`] reported. Input to `rename_apply`.
 */
export interface RenameSelection {
    fqn: string;
    line: number;
    col: number;
}

/**
 * One page in a [`DocGroupInput`]: its title, source path, and Markdown body.
 */
export interface DocItemInput {
    title?: string;
    path?: string;
    content?: string;
}

/**
 * One rendered site file returned by `render_doc_site`.
 */
export interface RenderedFile {
    path: string;
    contents: string;
}

/**
 * One vendored git-dependency file for `dependency_modules`: its
 * `pds_modules/` slug, its FQN within the dependency workspace (the host\'s
 * path→FQN derivation), and its source.
 */
export interface VendoredInput {
    slug: string;
    fqn: string;
    source: string;
}

/**
 * Per-diagram layout tweaks from the IDE\'s \"Layout\" toggles. Applies only to C4
 * views; other scene kinds ignore it.
 */
export interface LayoutTweaks {
    /**
     * Run the long-edge optimiser (same-rank moves minimising Σ edge-length²).
     */
    minimize_long_edges?: boolean;
    /**
     * Reading direction: `\"tb\"` (default) or `\"lr\"`.
     */
    orientation?: string | undefined;
    /**
     * Spacing preset: `\"compact\"`, `\"comfortable\"` (default), or `\"roomy\"`.
     */
    spacing?: string | undefined;
    /**
     * **Experimental**: brute-force grid placement instead of the layered engine.
     */
    experimental_grid?: boolean;
    /**
     * Grid-placement cost dials (used only when `experimental_grid`). Absent → the
     * engine default. Each is the weight of: a crossing, a cell of edge length,
     * and a cell travelled against the reading direction (directionality).
     */
    grid_crossing_cost?: number | undefined;
    grid_distance_cost?: number | undefined;
    grid_flow_cost?: number | undefined;
    /**
     * Grid search mode: `\"auto\"` (default), `\"heuristic\"`, or `\"exhaustive\"` — the
     * heuristic-vs-brute-force toggle, for checking the heuristic against exact.
     */
    grid_search?: string | undefined;
    /**
     * Nodes pinned to grid cells (drag-to-pin). Used only when `experimental_grid`;
     * the engine fixes these and searches only the rest. Pins for nodes not in the
     * current view are ignored.
     */
    grid_pins?: GridPin[];
}

/**
 * The `[doc]` table parsed from a `pds.toml` for the host: the sidebar groups
 * and their page entries (no content — the host loads the files the manifest
 * names, then hands the assembled config to `render_doc_site`).
 */
export interface DocManifest {
    name?: string;
    theme?: string;
    logo?: string;
    sidebar?: DocManifestGroup[];
}

/**
 * The `{ kind, value }` of an LSP `MarkupContent`.
 */
export interface MarkupContent {
    kind: string;
    value: string;
}

/**
 * The host\'s documentation config for `render_doc_site`: site name, optional
 * theme word (`dark`/`light`, default `dark`), optional logo path, and the
 * authored doc groups with their pages\' already-loaded Markdown `content`.
 */
export interface DocConfigInput {
    name: string;
    theme?: string | undefined;
    logo?: string | undefined;
    docs?: DocGroupInput[];
}

/**
 * The kind of construct a fold covers (§3.5/§5.1), so the editor can pick a
 * default fold state per kind — collapse `member` impl blocks on open, leave
 * the structural `node` bodies expanded.
 */
export type FoldKind = "node" | "member" | "data" | "block";

/**
 * The result of find-usages: the resolved symbol plus every occurrence.
 */
export interface References {
    fqn: string;
    title: string;
    occurrences: Occurrence[];
}

/**
 * The whole workspace as a software graph for the 3D relationship view.
 */
export interface UniverseSnapshot {
    nodes: UniverseNode[];
    edges: UniverseEdge[];
}


/**
 * The IDE session: the workspace state plus every language, diagram, and doc
 * query over it. The host holds one per open workspace.
 *
 * One cache, one staleness story: `modules` caches each module's parse (so an
 * edit re-parses only that module), and `built` memoises the resolved workspace
 * + graph. Any mutation clears `built`; the next query rebuilds it once and the
 * rest of a query burst between keystrokes reads it — nothing unchanged is ever
 * re-parsed or re-resolved.
 */
export class IdeSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Parse + static-check a single `source` buffer — the editor's per-keystroke
     * lint path (one module, no workspace context, so cheaper than
     * [`Self::diagnostics`]), independent of held state.
     */
    check(source: string): Diagnostic[];
    /**
     * Completions at byte `offset` in module `fqn`, resolved across the
     * workspace and its externals.
     */
    completion(fqn: string, offset: number): Completion[];
    /**
     * The FQN of the declaration of the symbol at `offset` in module `fqn`, for
     * go-to-definition; `None` when the cursor rests on no resolvable symbol.
     */
    definition(fqn: string, offset: number): string | undefined;
    /**
     * Resolves the consumer workspace's direct dependencies (§8.3) into
     * dependency-name-prefixed modules — the externals `mount` takes. `lock` is
     * the consumer's `pds.lock` (blank when absent); `vendored`/`local` are the
     * files the host read for vendored git deps and local-source deps (ADR-026).
     *
     * # Errors
     * Returns an error when `lock` is present but not valid TOML.
     */
    dependency_modules(lock: string, vendored: VendoredInput[], local: LocalInput[]): Module[];
    /**
     * Workspace-wide diagnostics, each module's problems attributed to it and
     * checked against the dependency externals so a `dep::module::Node`
     * reference resolves (§8.3).
     */
    diagnostics(): ModuleResult[];
    /**
     * Parses a `pds.toml` string into the doc manifest the host needs to build
     * the sidebar and read its pages. Uses the same `toml` parser as the CLI.
     *
     * # Errors
     * Returns an error when `toml` is not valid TOML of the `[doc]` shape.
     */
    doc_manifest(toml: string): DocManifest;
    /**
     * The Svelte SSR bundle (`ssr.js`) the host evaluates in its own JavaScript
     * engine to define `globalThis.SSR.renderPage` — hand that back to
     * [`Self::render_doc_site`] as the `render` callback.
     */
    doc_ssr_bundle(): string;
    /**
     * Projects a diagram `view` over the held workspace and returns the
     * [`Scene`] as JSON. `view` is `context`/`container`/`component`/`sequence`;
     * `target` is the boundary or entry FQN (ignored for `context`).
     *
     * # Errors
     * Returns an error for an unknown `view` or a view that cannot be projected.
     */
    emit_scene(view: string, target: string): string;
    /**
     * Foldable regions of a single `source` buffer (editor-local), each tagged
     * with its construct kind. 0-based lines; single-line spans are dropped (an
     * editor cannot fold them).
     */
    folding_ranges(source: string): FoldingRange[];
    /**
     * Canonical formatting of a single `source` buffer.
     *
     * # Errors
     * Returns an error when `source` does not parse.
     */
    format(source: string): string;
    /**
     * Markdown hover for the symbol at byte `offset` in module `fqn`.
     */
    hover(fqn: string, offset: number): Hover | undefined;
    /**
     * Positions a [`Scene`] (as JSON) into absolute coordinates, returning the
     * layout as JSON. The two layout shapes are distinguishable by their fields
     * (`participants` vs `nodes`). `tweaks` (optional) applies the C4 "Layout"
     * toggles; other scene kinds ignore it.
     *
     * # Errors
     * Returns an error for invalid JSON.
     */
    layout_scene(scene_json: string, tweaks?: LayoutTweaks | null): string;
    /**
     * Load the workspace: re-parse every module, replace the externals, and
     * invalidate the built workspace/graph.
     */
    mount(modules: Module[], externals: Module[]): void;
    /**
     * A new, empty session.
     */
    constructor();
    /**
     * The nodes declared across the workspace, for a diagram's target picker.
     */
    outline(): OutlineNode[];
    /**
     * Every occurrence of the symbol at `offset` in module `fqn` across the
     * workspace — find-usages. `None` when the cursor rests on no symbol.
     */
    references(fqn: string, offset: number): References | undefined;
    /**
     * Renames the symbol at `offset` in module `fqn` to `new_name`, applying
     * only the `selected` occurrences (by their 1-based `line`/`col`). Returns
     * the new full source of every module that changed.
     *
     * # Errors
     * Returns an error when `new_name` is not a valid identifier.
     */
    rename_apply(fqn: string, offset: number, new_name: string, selected: RenameSelection[]): RenamedSource[];
    /**
     * Renders the whole documentation site for the held workspace, exactly as
     * `pds doc` does, driving SSR through the host's JS `render` callback
     * (`(propsJson) => head/body JSON`, typically `SSR.renderPage`).
     *
     * # Errors
     * Returns an error when a page fails to render (a bundle/engine defect).
     */
    render_doc_site(config: DocConfigInput, render: Function): RenderedFile[];
    /**
     * Semantic tokens for a single `source` buffer (editor-local).
     */
    semantic_tokens(source: string): SemanticTokens;
    /**
     * Apply an edit: re-parse only this module and invalidate the built
     * workspace/graph. An unchanged buffer (a defensive re-push) is a no-op, so
     * the cache stays warm for the query burst that follows.
     */
    set_source(fqn: string, text: string): void;
    /**
     * Projects the fitting diagram for the symbol `fqn` over the held workspace
     * and returns its [`Scene`] as JSON (the side-panel counterpart of `hover`).
     *
     * # Errors
     * Returns an error for an unknown symbol or one that cannot be projected.
     */
    symbol_scene(fqn: string): string;
    /**
     * The whole workspace as a software graph for the 3D relationship view: nodes
     * (systems, containers, components, people) with containment, and directed
     * relationships weighted by traffic. The renderer lays it out (d3-force-3d)
     * client-side.
     */
    universe(): UniverseSnapshot;
}

/**
 * The crate version, for host-side compatibility checks.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_idesession_free: (a: number, b: number) => void;
    readonly idesession_check: (a: number, b: number, c: number) => [number, number];
    readonly idesession_completion: (a: number, b: number, c: number, d: number) => [number, number];
    readonly idesession_definition: (a: number, b: number, c: number, d: number) => [number, number];
    readonly idesession_dependency_modules: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly idesession_diagnostics: (a: number) => [number, number];
    readonly idesession_doc_manifest: (a: number, b: number, c: number) => [number, number, number];
    readonly idesession_doc_ssr_bundle: (a: number) => [number, number];
    readonly idesession_emit_scene: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly idesession_folding_ranges: (a: number, b: number, c: number) => [number, number];
    readonly idesession_format: (a: number, b: number, c: number) => [number, number, number, number];
    readonly idesession_hover: (a: number, b: number, c: number, d: number) => any;
    readonly idesession_layout_scene: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly idesession_mount: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly idesession_new: () => number;
    readonly idesession_outline: (a: number) => [number, number];
    readonly idesession_references: (a: number, b: number, c: number, d: number) => any;
    readonly idesession_rename_apply: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly idesession_render_doc_site: (a: number, b: any, c: any) => [number, number, number, number];
    readonly idesession_semantic_tokens: (a: number, b: number, c: number) => any;
    readonly idesession_set_source: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly idesession_symbol_scene: (a: number, b: number, c: number) => [number, number, number, number];
    readonly idesession_universe: (a: number) => any;
    readonly version: () => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
