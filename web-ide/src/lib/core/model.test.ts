import { describe, expect, it } from "vitest";

import {
  ancestry,
  buildModelIndex,
  nodeByteOffset,
  nodeTitle,
  ownerNodeOf,
  resolveNodeFqn,
  singleLifelineScene,
} from "./model.js";
import type { StructureNode } from "./types.js";

const node = (fqn: string, kind: string, parent: string | null = null, name = fqn.split("::").at(-1)!): StructureNode => ({
  fqn,
  name,
  kind,
  triggered: false,
  line: 1,
  col: 1,
  parent,
});

// Two modules with the same path prefix — the longest-prefix grouping must not
// mis-file `m::core::Exchange` under `m`.
const nodes: StructureNode[] = [
  node("m::Sys", "system"),
  node("m::core::Exchange", "container", "m::Sys"),
  node("m::core::Exchange::Run", "callable", "m::core::Exchange"),
  node("m::core::Conv", "data"),
];
const moduleFqns = ["m", "m::core"];

describe("buildModelIndex", () => {
  const idx = buildModelIndex(nodes, moduleFqns);

  it("groups nodes to their file by the longest module-FQN prefix", () => {
    expect(idx.structureByFile["m"].map((n) => n.fqn)).toEqual(["m::Sys"]);
    expect(idx.structureByFile["m::core"].map((n) => n.fqn)).toEqual([
      "m::core::Exchange",
      "m::core::Exchange::Run",
      "m::core::Conv",
    ]);
  });

  it("keys nodeIndex by FQN with its declaring file", () => {
    expect(idx.nodeIndex.get("m::core::Exchange")?.fileFqn).toBe("m::core");
    expect(idx.symbols.find((s) => s.fqn === "m::core::Exchange")?.fileFqn).toBe("m::core");
  });

  it("buckets callables as flows under their owner, and maps types/info", () => {
    expect(idx.flowsByNode.get("m::core::Exchange")?.map((f) => f.name)).toEqual(["Run"]);
    expect(idx.typeFqnByName["Conv"]).toBe("m::core::Conv");
    expect(idx.nodeInfo["m::core::Exchange"]).toEqual({ kind: "container", parent: "m::Sys" });
  });
});

describe("resolvers", () => {
  const idx = buildModelIndex(nodes, moduleFqns);

  it("resolveNodeFqn returns a direct node, or a collapsed member's real node", () => {
    expect(resolveNodeFqn(idx, "m::core::Exchange")).toBe("m::core::Exchange");
    // a call folded to its system owner still resolves to the real callable
    expect(resolveNodeFqn(idx, "m::Sys::Run")).toBe("m::core::Exchange::Run");
    expect(resolveNodeFqn(idx, "m::Nope::ghost")).toBeNull();
  });

  it("ownerNodeOf walks up to the nearest node (field → owner)", () => {
    expect(ownerNodeOf(idx, "m::core::Conv::id")).toBe("m::core::Conv");
    expect(ownerNodeOf(idx, "m::core::Exchange")).toBeNull(); // already a node, no ancestor
  });

  it("ancestry follows parent to the root", () => {
    expect(ancestry(idx, "m::core::Exchange::Run")).toEqual([
      "m::Sys",
      "m::core::Exchange",
      "m::core::Exchange::Run",
    ]);
  });

  it("nodeTitle renders kind + name, falling back to the leaf", () => {
    expect(nodeTitle(idx, "m::core::Exchange")).toBe("container `Exchange`");
    expect(nodeTitle(idx, "m::ghost")).toBe("`ghost`");
  });

  it("singleLifelineScene builds a one-participant sequence", () => {
    const s = singleLifelineScene(idx, "m::core::Exchange::Run") as Record<string, unknown>;
    expect(s.view).toBe("sequence");
    expect(s.entry).toBe("m::core::Exchange::Run");
    expect((s.participants as { kind: string }[])[0].kind).toBe("callable");
  });
});

describe("nodeByteOffset", () => {
  it("maps 1-based line/col to a byte offset", () => {
    const src = "//! m\npublic system S;\n";
    expect(nodeByteOffset(src, 1, 1)).toBe(0);
    expect(nodeByteOffset(src, 2, 8)).toBe(6 + 7); // line 2 starts at byte 6, col 8 → +7
  });
});
