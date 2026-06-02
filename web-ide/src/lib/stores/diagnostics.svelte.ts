// Workspace diagnostics — a derived rune store (owns no state).
//
// Re-runs the static check whenever the workspace buffers change, via the pure
// core/diagnostics over wsStore + the WASM `checkModules`. Exposes problems /
// errorCount / errorPaths for the problems pane and the file-tree markers.

import { computeDiagnostics, type DiagnosticsResult } from "$lib/core/diagnostics.js";
import { checkModules } from "$lib/pds.js";
import { wasm } from "./wasm.svelte.js";
import { wsStore } from "./workspace.svelte.js";

const EMPTY: DiagnosticsResult = { results: null, problems: [], errorCount: 0, errorPaths: new Set() };

class DiagnosticsStore {
  readonly result = $derived.by<DiagnosticsResult>(() => {
    const ws = wsStore.workspace;
    if (!wasm.ready || !ws) return EMPTY;
    return computeDiagnostics(wsStore.allModules, ws.files, checkModules);
  });

  get problems() {
    return this.result.problems;
  }
  get errorCount() {
    return this.result.errorCount;
  }
  get errorPaths() {
    return this.result.errorPaths;
  }
}

export const diagnostics = new DiagnosticsStore();
