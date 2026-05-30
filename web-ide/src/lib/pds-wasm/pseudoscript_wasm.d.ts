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
 * Projects a diagram view from `source` and renders it to a self-contained SVG
 * string. See [`emit_scene`] for the `view`/`target` arguments.
 *
 * # Errors
 *
 * Returns an error for an unknown `view`, or when the view cannot be projected.
 */
export function emit_svg(source: string, view: string, target: string): string;

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
 * Lists the nodes declared in `source` as a JSON array of
 * `{ fqn, name, kind, triggered }`. A host uses this to populate a diagram's
 * target picker: `container` views target a `system`, `component` views a
 * `container`, and `sequence` views a `triggered` callable.
 */
export function outline(source: string): string;

/**
 * Parses `source` and returns its **syntax** diagnostics as a JSON array.
 * Faster than [`check`] — no static analysis — for an editor's parse-error
 * squiggles on every keystroke.
 */
export function parse(source: string): string;

/**
 * Routes Rust panics to the browser console with a readable stack. Runs once
 * on module instantiation (wasm only).
 */
export function start(): void;

/**
 * The crate version, for host-side compatibility checks.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly check: (a: number, b: number) => [number, number];
    readonly check_modules: (a: number, b: number) => [number, number, number, number];
    readonly emit_scene: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly emit_svg: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly format: (a: number, b: number) => [number, number, number, number];
    readonly outline: (a: number, b: number) => [number, number];
    readonly parse: (a: number, b: number) => [number, number];
    readonly version: () => [number, number];
    readonly start: () => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
