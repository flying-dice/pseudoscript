import { describe, expect, it, vi } from "vitest";

import { canvasHint, projectCanvas, type CanvasWasm } from "./canvas.js";
import { buildModelIndex } from "./model.js";
import type { Scene, StructureNode } from "./types.js";

const idx = buildModelIndex(
  [{ fqn: "m::C", name: "C", kind: "container", triggered: false, line: 1, col: 1, parent: null } as StructureNode],
  ["m"],
);

const seqScene = (participants: unknown[]): Scene => ({ view: "sequence", participants, items: [] });
const c4Scene = (nodes: unknown[]): Scene => ({ view: "context", nodes, edges: [] });

const wasm = (over: Partial<CanvasWasm> = {}): CanvasWasm => ({
  symbolScene: () => seqScene([{ fqn: "m::C" }, { fqn: "x" }]),
  emitSceneModules: () => c4Scene([{ fqn: "m::C" }]),
  layoutScene: (s) => ({ ...s, laid: true }),
  ...over,
});

describe("projectCanvas", () => {
  it("lays out a sequence scene for a selected symbol", () => {
    const r = projectCanvas({ selected: { fqn: "m::C" }, seqDepth: "component", modules: [], index: idx, wasm: wasm(), onError: () => {} });
    expect(r.error).toBe("");
    expect((r.layout as { laid?: boolean })?.laid).toBe(true);
  });

  it("projects the context overview with no selection (no layout)", () => {
    const r = projectCanvas({ selected: null, seqDepth: "component", modules: [], index: idx, wasm: wasm(), onError: () => {} });
    expect((r.scene as { nodes: unknown[] }).nodes).toHaveLength(1);
    expect(r.layout).toBeNull();
  });

  it("falls back to a lifeline when a selected sequence is empty", () => {
    const r = projectCanvas({
      selected: { fqn: "m::C" },
      seqDepth: "component",
      modules: [],
      index: idx,
      wasm: wasm({ symbolScene: () => seqScene([]) }),
      onError: () => {},
    });
    expect((r.scene as { participants: unknown[] }).participants).toHaveLength(1);
  });

  it("on a throw with a selection, reports and falls back to a lifeline", () => {
    const onError = vi.fn();
    const r = projectCanvas({
      selected: { fqn: "m::C" },
      seqDepth: "component",
      modules: [],
      index: idx,
      wasm: wasm({ symbolScene: () => { throw new Error("nope"); } }),
      onError,
    });
    expect(onError).toHaveBeenCalledWith("DIAGRAM_PROJECTION_FAILED", expect.stringContaining("nope"));
    expect((r.scene as { view: string }).view).toBe("sequence"); // lifeline fallback
  });

  it("on a throw with no selection, reports an error result", () => {
    const onError = vi.fn();
    const r = projectCanvas({
      selected: null,
      seqDepth: "component",
      modules: [],
      index: idx,
      wasm: wasm({ emitSceneModules: () => { throw new Error("boom"); } }),
      onError,
    });
    expect(onError).toHaveBeenCalledWith("DIAGRAM_RENDER_FAILED", "boom");
    expect(r.scene).toBeNull();
    expect(r.error).toBe("boom");
  });
});

describe("canvasHint", () => {
  it("differs for a selection vs the whole model", () => {
    expect(canvasHint({ fqn: "x" })).toContain("Nothing to draw");
    expect(canvasHint(null)).toContain("context overview");
  });
});
