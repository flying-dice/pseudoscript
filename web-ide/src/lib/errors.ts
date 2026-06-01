// Central error-code registry for the IDE.
//
// Every recoverable failure or "nothing happened" path reports through here so
// the console carries a stable, greppable code (`PDS-<AREA>-<NNN>`) instead of
// failing silently. Codes group by origin (GOTO, DIAGRAM, WASM, WORKSPACE), so
// a console scan reveals the failure pattern and where it came from. Reports are
// also retained in a small ring buffer (`errorLog`) for a future diagnostics
// view and for assertions in tests.

export type Severity = "error" | "warn";

export interface ErrorDef {
  // Stable wire identifier surfaced to the console — `PDS-<AREA>-<NNN>`.
  code: string;
  // One-line description of the failure.
  title: string;
  severity: Severity;
}

// The registry. Keys are the symbolic names used at call sites; `code` is the
// stable identifier. Add a new entry rather than reusing a code for a new cause
// — codes are how failures are correlated across sessions.
export const CODES = {
  // ── Go-to-definition / navigation ──────────────────────────────────────────
  GOTO_NO_SYMBOL: {
    code: "PDS-GOTO-001",
    title: "Go-to-definition found no resolvable symbol under the cursor",
    severity: "warn",
  },
  GOTO_MEMBER_FALLBACK: {
    code: "PDS-GOTO-002",
    title: "Go-to-definition landed on a member; navigated to its owner instead",
    severity: "warn",
  },
  GOTO_UNRESOLVED: {
    code: "PDS-GOTO-003",
    title: "Go-to-definition resolved to a symbol the navigator could not open",
    severity: "error",
  },
  GOTO_FILE_MISSING: {
    code: "PDS-GOTO-004",
    title: "Go-to-definition target's declaring file is not in the workspace",
    severity: "error",
  },

  // ── Diagram projection / canvas ────────────────────────────────────────────
  DIAGRAM_PROJECTION_FAILED: {
    code: "PDS-DIAGRAM-001",
    title: "Symbol could not be projected to a diagram; using lifeline fallback",
    severity: "warn",
  },
  DIAGRAM_RENDER_FAILED: {
    code: "PDS-DIAGRAM-002",
    title: "Whole-model diagram could not be rendered",
    severity: "error",
  },

  // ── Language-server (WASM) bridge ──────────────────────────────────────────
  WASM_CALL_FAILED: {
    code: "PDS-WASM-001",
    title: "A language-server call threw",
    severity: "error",
  },

  // ── Workspace / file IO ────────────────────────────────────────────────────
  WORKSPACE_IO_FAILED: {
    code: "PDS-WORKSPACE-001",
    title: "A workspace file operation failed",
    severity: "error",
  },
} satisfies Record<string, ErrorDef>;

export type CodeName = keyof typeof CODES;

export interface ErrorReport extends ErrorDef {
  // The symbolic registry key.
  name: CodeName;
  // Free-form specifics (the offending fqn, file, call name, …).
  detail?: string;
  // Structured context for inspection in the console / diagnostics view.
  context?: Record<string, unknown>;
  // Epoch milliseconds the report was raised.
  at: number;
}

const RING_MAX = 200;
const ring: ErrorReport[] = [];
const listeners = new Set<(report: ErrorReport) => void>();

// Report a coded failure: logs to the console (error vs warn per the code),
// retains it in the ring buffer, and notifies listeners. Returns the report so
// callers can attach it to UI state. Never throws.
export function reportError(name: CodeName, detail?: string, context?: Record<string, unknown>): ErrorReport {
  const def = CODES[name];
  const report: ErrorReport = { ...def, name, detail, context, at: Date.now() };

  ring.push(report);
  if (ring.length > RING_MAX) ring.shift();

  const line = `[${def.code}] ${def.title}${detail ? ` — ${detail}` : ""}`;
  if (def.severity === "error") console.error(line, context ?? "");
  else console.warn(line, context ?? "");

  for (const fn of listeners) fn(report);
  return report;
}

// The retained reports, oldest first — for a diagnostics view or test assertions.
export function errorLog(): readonly ErrorReport[] {
  return ring;
}

// Drop every retained report (used by tests and a future "clear" action).
export function clearErrorLog(): void {
  ring.length = 0;
}

// Subscribe to reports as they are raised. Returns an unsubscribe function.
export function onError(fn: (report: ErrorReport) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
