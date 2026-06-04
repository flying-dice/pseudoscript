// Workspace diagnostics — a derived rune store (owns no state).
//
// Re-runs the static check whenever the workspace buffers change, via the pure
// core/diagnostics over wsStore + the WASM `checkModules`. Exposes problems /
// errorCount / errorPaths for the problems pane and the file-tree markers.

import { computeDiagnostics, type DiagnosticsResult } from "$lib/core/diagnostics.js";
import { ideDiagnostics } from "$lib/pds.js";
import { sessionMount } from "./session.svelte.js";
import { wasm } from "./wasm.svelte.js";
import { wsStore } from "./workspace.svelte.js";

const EMPTY: DiagnosticsResult = { results: null, problems: [], errorCount: 0, errorPaths: new Set() };

class DiagnosticsStore {
  readonly result = $derived.by<DiagnosticsResult>(() => {
    const ws = wsStore.workspace;
    if (!wasm.ready || !ws) return EMPTY;
    // Bind dependency modules as externals so a cross-workspace reference (§8.3)
    // resolves instead of diagnosing as unknown.
    // `allModules` keeps this reactive on every edit (the session is current via
    // `setIdeSource`); `sessionMount.seq` re-runs it once a structural mount has
    // loaded the new modules (the mount runs in an effect, after this derived's
    // first read). The query itself takes no modules — the session holds them.
    void sessionMount.seq;
    return computeDiagnostics(wsStore.allModules, ws.files, () => ideDiagnostics());
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
