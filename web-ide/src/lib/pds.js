// Loader + typed wrappers around the PseudoScript compiler wasm module.
//
// The vendored `pds-wasm/` package is wasm-bindgen's `--target web` output:
// the default export initialises the module (fetching the `.wasm`), after which
// the named functions are synchronous. Call `initWasm()` once before using them.
import init, {
  check as wasmCheck,
  parse as wasmParse,
  format as wasmFormat,
  emit_scene as wasmEmitScene,
  emit_svg as wasmEmitSvg,
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

/** Format source to canonical form; throws on a parse error. */
export function format(source) {
  return wasmFormat(source);
}

/** Project a diagram view to its laid-out scene object. */
export function emitScene(source, view, target = "") {
  return JSON.parse(wasmEmitScene(source, view, target));
}

/** Project a diagram view to an SVG string. */
export function emitSvg(source, view, target = "") {
  return wasmEmitSvg(source, view, target);
}

/** The compiler crate version. */
export function version() {
  return wasmVersion();
}
