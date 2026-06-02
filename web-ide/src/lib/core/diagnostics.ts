// Workspace diagnostics — derive the problem list, error count, and error-marked
// file paths from a static check of every module. Pure: the WASM `checkModules`
// is injected, and a throw degrades to "no results" (the view stays usable).

import type { Module, OpenFile, Problem } from "./types.js";

// A module's check result: its FQN and raw diagnostics. Structurally matches
// `pds.ts`'s `ModuleDiagnostics`, so the real `checkModules` is assignable.
type ModuleResult = { fqn: string; diagnostics: ReadonlyArray<{ severity: string }> };

export type DiagnosticsResult = {
  // Per-module raw results, or null when the check threw / isn't ready.
  results: ReadonlyArray<ModuleResult> | null;
  // Every diagnostic, tagged with its owning module's FQN.
  problems: Problem[];
  // Count of error-severity problems.
  errorCount: number;
  // Tree paths of files that have at least one error (for the file-tree marker).
  errorPaths: Set<string>;
};

const EMPTY: DiagnosticsResult = { results: null, problems: [], errorCount: 0, errorPaths: new Set() };

/**
 * Check `modules` and shape the result for the view. `files` maps FQNs to disk
 * paths for the error-path set. A `check` throw yields the empty result.
 */
export function computeDiagnostics(
  modules: Module[],
  files: OpenFile[],
  check: (modules: Module[]) => ReadonlyArray<ModuleResult>,
): DiagnosticsResult {
  let results: ReadonlyArray<ModuleResult>;
  try {
    results = check(modules);
  } catch {
    return EMPTY;
  }

  const problems: Problem[] = results.flatMap((m) =>
    (m.diagnostics as unknown as Problem[]).map((d) => ({ ...d, file: m.fqn })),
  );
  const errorCount = problems.filter((d) => d.severity === "error").length;

  const byFqn = new Map<string, string | undefined>();
  for (const f of files) if (f.fqn) byFqn.set(f.fqn, f.path);
  const errorPaths = new Set<string>();
  for (const m of results) {
    if (m.diagnostics.some((d) => d.severity === "error")) {
      const p = byFqn.get(m.fqn);
      if (p) errorPaths.add(p);
    }
  }

  return { results, problems, errorCount, errorPaths };
}
