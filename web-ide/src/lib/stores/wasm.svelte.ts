// WASM lifecycle — a reactive rune store.
//
// The one place that initialises the compiler module and tracks readiness. The
// view gates rendering on `wasm.ready`, shows `wasm.error` on init failure, and
// displays `wasm.version`. The language functions themselves stay imported from
// `$lib/pds.js` at the call sites (the page is the composition root); core
// modules receive them as a `WasmApi` value.

import { initWasm, version } from "$lib/pds.js";

class WasmStore {
  // Whether the module is initialised and the IDE may render its workspace.
  ready = $state(false);
  // The init error message, or null.
  error = $state<string | null>(null);
  // The compiler crate version.
  version = $state("");

  /** Initialise the module and read its version. Returns false (and sets
   *  `error`) on failure; the caller flips `ready` once the rest of boot is set. */
  async init(): Promise<boolean> {
    this.error = null;
    try {
      await initWasm();
    } catch (e) {
      this.error = String((e as Error)?.message ?? e);
      return false;
    }
    this.version = version();
    return true;
  }
}

export const wasm = new WasmStore();
