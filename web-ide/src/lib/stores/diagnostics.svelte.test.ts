import { flushSync } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";

// The diagnostics store derives from the wasm check; stub it. (wasm.svelte.js also
// imports initWasm/version from pds.js, unused here — we drive wasm.ready directly.)
const { ideDiagnostics } = vi.hoisted(() => ({ ideDiagnostics: vi.fn(() => [] as unknown[]) }));
vi.mock("$lib/pds.js", () => ({ ideDiagnostics, initWasm: vi.fn(), version: vi.fn(() => "") }));

import type { WorkspaceModel } from "$lib/core/types.js";
import { diagnostics } from "./diagnostics.svelte.js";
import { sessionMount } from "./session.svelte.js";
import { wasm } from "./wasm.svelte.js";
import { wsStore } from "./workspace.svelte.js";

afterEach(() => {
  wasm.ready = false;
  wsStore.workspace = null;
  wsStore.moduleSources = {};
  sessionMount.seq = 0;
  ideDiagnostics.mockReset().mockReturnValue([]);
});

// Reads the derived inside a live effect root, flushing so it recomputes.
function read<T>(get: () => T): T {
  let captured!: T;
  const stop = $effect.root(() => {
    $effect(() => {
      captured = get();
    });
  });
  flushSync();
  stop();
  return captured;
}

const workspace = (): WorkspaceModel =>
  ({ files: [{ fqn: "a", path: "a.pds" }] }) as unknown as WorkspaceModel;

describe("DiagnosticsStore", () => {
  it("is empty before wasm is ready", () => {
    wsStore.workspace = workspace();
    expect(read(() => diagnostics.problems)).toEqual([]);
    expect(read(() => diagnostics.errorCount)).toBe(0);
    expect(read(() => diagnostics.results)).toBeNull();
  });

  it("is empty with no workspace", () => {
    wasm.ready = true;
    expect(read(() => diagnostics.results)).toBeNull();
  });

  it("flows check results through when ready with a workspace", () => {
    wasm.ready = true;
    wsStore.workspace = workspace();
    ideDiagnostics.mockReturnValue([
      { fqn: "a", diagnostics: [{ severity: "error", message: "boom", start_line: 1, start_col: 1 }] },
    ] as never);
    expect(read(() => diagnostics.errorCount)).toBe(1);
    expect(read(() => diagnostics.problems)[0]).toMatchObject({ file: "a", message: "boom" });
    expect([...read(() => diagnostics.errorPaths)]).toEqual(["a.pds"]);
    expect(read(() => diagnostics.results)).not.toBeNull();
  });

  it("re-derives after a structural remount bump", () => {
    wasm.ready = true;
    wsStore.workspace = workspace();
    ideDiagnostics.mockReturnValue([]);
    expect(read(() => diagnostics.errorCount)).toBe(0);
    ideDiagnostics.mockReturnValue([
      { fqn: "a", diagnostics: [{ severity: "error", message: "x", start_line: 1, start_col: 1 }] },
    ] as never);
    sessionMount.bump();
    expect(read(() => diagnostics.errorCount)).toBe(1);
  });
});
