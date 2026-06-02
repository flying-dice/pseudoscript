/* tslint:disable */
/* eslint-disable */

/**
 * Parses and statically checks `source` as a single module, returning every
 * diagnostic (parse errors then static errors) as a JSON array.
 */
export function check(source: string): string;

/**
 * Checks a multi-module workspace. `modules_json` is a JSON array of
 * `{ "fqn": string, "source": string }`. Returns a JSON array of
 * `{ "fqn": string, "diagnostics": Diagnostic[] }`, with each module's
 * diagnostics attributed to it (cross-module errors land on the referring
 * module, §8.2).
 *
 * # Errors
 *
 * Returns an error when `modules_json` is not valid JSON of the expected
 * shape.
 */
export function check_modules(modules_json: string): string;

/**
 * Context-aware completion at `offset` (a byte offset) in module `module_fqn`,
 * as a JSON array of LSP `CompletionItem`s (`{label, kind, detail}`, where
 * `kind` is the integer `CompletionItemKind`). Scoped to the trigger before the
 * caret (`.`/`::`/`#[`/type-position/general); the client filters against the
 * typed prefix. Served by the shared [`pseudoscript_lsp_core::complete`] —
 * identical to the stdio server's `textDocument/completion`. `modules_json` is
 * the `[{fqn, source}]` workspace shape.
 *
 * # Errors
 *
 * Returns an error when `modules_json` is not valid JSON of the expected shape.
 */
export function completion(modules_json: string, module_fqn: string, offset: number): string;

/**
 * Resolves the symbol under `offset` (a byte offset) in module `module_fqn` to
 * the FQN of its declaration, for go-to-definition. Returns the FQN as a JSON
 * string, or `null` when the cursor rests on no resolvable symbol. Unlike
 * [`hover`] it renders no diagram, so it is cheap enough for a click handler.
 * `modules_json` is the `[{fqn, source}]` workspace shape.
 *
 * # Errors
 *
 * Returns an error when `modules_json` is not valid JSON of the expected shape.
 */
export function definition(modules_json: string, module_fqn: string, offset: number): string;

/**
 * Parses a `pds.toml` string into the doc manifest the host needs to build the
 * sidebar and read its pages: JSON
 * `{ name?, theme?, logo?, lang?, sidebar: [{ title, items: [{ title, path }] }] }`.
 * The host loads each `path`, then hands the assembled config (with page
 * `content`) back to [`render_doc_site`]. Uses the same `toml` parser as the
 * native CLI, so the two agree on the schema.
 *
 * # Errors
 *
 * Returns an error when `toml` is not valid TOML of the `[doc]` shape.
 */
export function doc_manifest(toml: string): string;

/**
 * The Svelte SSR bundle (`ssr.js`) the host evaluates in its own JavaScript
 * engine — the browser — to define `globalThis.SSR.renderPage`. Hand that
 * function back to [`render_doc_site`] as the `render` callback.
 */
export function doc_ssr_bundle(): string;

/**
 * Projects a diagram view from `source` and returns the laid-out [`Scene`] as
 * JSON. `view` is one of `context`, `container`, `component`, or `sequence`;
 * `target` is the boundary FQN (container/component) or entry callable FQN
 * (sequence), and is ignored for `context`.
 *
 * # Errors
 *
 * Returns an error for an unknown `view`, or when the view cannot be projected
 * (the target resolves to no node, or the wrong kind).
 */
export function emit_scene(source: string, view: string, target: string): string;

/**
 * Projects a diagram view over a whole workspace graph, so it shows nodes and
 * edges across modules (a container's components, cross-system calls). Same
 * `view`/`target` arguments as [`emit_scene`]; `modules_json` is `[{fqn,
 * source}]`.
 *
 * # Errors
 *
 * Returns an error for invalid JSON, an unknown `view`, or a view that cannot
 * be projected.
 */
export function emit_scene_modules(modules_json: string, view: string, target: string): string;

/**
 * Projects a diagram view from `source` and renders it to a self-contained SVG
 * string. See [`emit_scene`] for the `view`/`target` arguments.
 *
 * # Errors
 *
 * Returns an error for an unknown `view`, or when the view cannot be projected.
 */
export function emit_svg(source: string, view: string, target: string): string;

/**
 * Foldable regions of `source` as the JSON of an LSP `FoldingRange` array
 * (`{ startLine, endLine, kind }`, 0-based lines) — every multi-line
 * declaration and statement block. Identical to the stdio server's
 * `textDocument/foldingRange` response; the editor folds these instead of
 * brace-matching in JS.
 */
export function folding_ranges(source: string): string;

/**
 * Formats `source` into its canonical form.
 *
 * # Errors
 *
 * Returns an error when `source` does not parse (formatting requires a valid
 * parse tree).
 */
export function format(source: string): string;

/**
 * Resolves the symbol under `offset` (a byte offset) in module `module_fqn` and
 * returns it as an LSP `Hover` (`{ contents: { kind, value }, range }`,
 * Markdown), or `null` when the cursor rests on no resolvable symbol. Served by
 * the shared [`pseudoscript_lsp_core::analysis::hover`] — identical to the
 * stdio server's `textDocument/hover`, no diagram. The interactive diagram is a
 * separate concern: [`symbol_scene`] / [`symbol_svg`]. `modules_json` is the
 * `[{fqn, source}]` workspace shape.
 *
 * # Errors
 *
 * Returns an error when `modules_json` is not valid JSON of the expected shape.
 */
export function hover(modules_json: string, module_fqn: string, offset: number): string;

/**
 * Positions a sequence [`Scene`] (as JSON) into absolute coordinates, returning
 * the layout as JSON. The host collapses the scene to a chosen depth first,
 * then hands it here; the layout engine owns all geometry. A non-sequence scene
 * is an error.
 *
 * # Errors
 *
 * Returns an error for invalid JSON or a non-sequence scene.
 */
export function layout_scene(scene_json: string): string;

/**
 * Lists the nodes declared in `source` as a JSON array of
 * `{ fqn, name, kind, triggered }`. A host uses this to populate a diagram's
 * target picker: `container` views target a `system`, `component` views a
 * `container`, and `sequence` views a `triggered` callable.
 */
export function outline(source: string): string;

/**
 * Like [`outline`], but over a whole workspace (`modules_json` is the same
 * `[{fqn, source}]` shape as [`check_modules`]), so a cross-module container or
 * system is a valid diagram target.
 *
 * # Errors
 *
 * Returns an error when `modules_json` is not valid JSON of the expected shape.
 */
export function outline_modules(modules_json: string): string;

/**
 * Parses `source` and returns its **syntax** diagnostics as a JSON array.
 * Faster than [`check`] — no static analysis — for an editor's parse-error
 * squiggles on every keystroke.
 */
export function parse(source: string): string;

/**
 * Finds every occurrence of the symbol under `offset` in module `module_fqn`
 * across the whole workspace — find-usages. Returns JSON
 * `{ fqn, title, occurrences: [{ fqn, line, col, end_line, end_col, text, decl }] }`,
 * where each occurrence carries its 1-based position, the trimmed source line
 * for a preview, and `decl` marking the declaration site. Returns `null` when
 * the cursor rests on no resolvable symbol. `modules_json` is `[{fqn, source}]`.
 *
 * # Errors
 *
 * Returns an error when `modules_json` is not valid JSON of the expected shape.
 */
export function references(modules_json: string, module_fqn: string, offset: number): string;

/**
 * Renames the symbol under `offset` in module `module_fqn` to `new_name`,
 * applying only the occurrences in `selected_json` — a JSON array of
 * `{fqn, line, col}` (1-based, matching [`references`]'s occurrence positions).
 * Returns JSON `[{ fqn, source }]`: the new full source of every module that
 * changed. The host swaps these into its buffers. The substitution is done here
 * (over UTF-8 byte spans) so the host never does offset math. Occurrence spans
 * come from the shared [`pseudoscript_lsp_core::refs::rename`].
 *
 * # Errors
 *
 * Returns an error when `new_name` is not a valid identifier, or when either
 * JSON argument is malformed.
 */
export function rename_apply(modules_json: string, module_fqn: string, offset: number, new_name: string, selected_json: string): string;

/**
 * Renders the whole documentation site for a workspace, exactly as the CLI's
 * `pds doc` does, driving server-side rendering through the host's JavaScript
 * engine rather than an embedded one.
 *
 * `render` is a JS function `(propsJson: string) => string` returning one
 * page's `{head, body}` JSON — typically `SSR.renderPage` from the evaluated
 * [`doc_ssr_bundle`]. `config_json` is `{ name, theme?, logo? }`. Returns the
 * site as JSON `[{ path, contents }]` for the host to write.
 *
 * # Errors
 *
 * Returns an error for invalid `modules_json`/`config_json`, or when a page
 * fails to render (a bundle/engine defect — not user model data).
 */
export function render_doc_site(modules_json: string, config_json: string, render: Function): string;

/**
 * AST-aware semantic tokens for `source`, as the JSON of an LSP
 * `SemanticTokens` (the delta-encoded `data` array over UTF-16 positions; the
 * `token_type` field indexes the [`pseudoscript_lsp_core::semantic`] legend).
 * Identical to the stdio server's `textDocument/semanticTokens/full` response —
 * the editor decodes and decorates it, replacing any hand-written tokenizer.
 */
export function semantic_tokens(source: string): string;

/**
 * Routes Rust panics to the browser console with a readable stack. Runs once
 * on module instantiation (wasm only).
 */
export function start(): void;

/**
 * Projects the fitting diagram for the symbol `fqn` over the whole workspace
 * and returns its laid-out [`Scene`] as JSON (the interactive counterpart of
 * [`hover`]'s `svg`, for a side panel or full-screen view). See
 * [`project_symbol`] for how the view is chosen.
 *
 * # Errors
 *
 * Returns an error for invalid JSON, an unknown symbol, or a symbol that
 * cannot be projected.
 */
export function symbol_scene(modules_json: string, fqn: string): string;

/**
 * Renders the fitting diagram for the symbol `fqn` (see [`project_symbol`]) to
 * a self-contained SVG string over the whole workspace — the live, re-derivable
 * form of [`hover`]'s `svg` for a docked side panel. `modules_json` is `[{fqn,
 * source}]`.
 *
 * # Errors
 *
 * Returns an error for invalid JSON, an unknown symbol, or a symbol that
 * cannot be projected.
 */
export function symbol_svg(modules_json: string, fqn: string): string;

/**
 * The crate version, for host-side compatibility checks.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly check: (a: number, b: number) => [number, number];
    readonly check_modules: (a: number, b: number) => [number, number, number, number];
    readonly completion: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly definition: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly doc_manifest: (a: number, b: number) => [number, number, number, number];
    readonly doc_ssr_bundle: () => [number, number];
    readonly emit_scene: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly emit_scene_modules: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly emit_svg: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly folding_ranges: (a: number, b: number) => [number, number];
    readonly format: (a: number, b: number) => [number, number, number, number];
    readonly hover: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly layout_scene: (a: number, b: number) => [number, number, number, number];
    readonly outline: (a: number, b: number) => [number, number];
    readonly outline_modules: (a: number, b: number) => [number, number, number, number];
    readonly parse: (a: number, b: number) => [number, number];
    readonly references: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly rename_apply: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
    readonly render_doc_site: (a: number, b: number, c: number, d: number, e: any) => [number, number, number, number];
    readonly semantic_tokens: (a: number, b: number) => [number, number];
    readonly symbol_scene: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly symbol_svg: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly version: () => [number, number];
    readonly start: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
