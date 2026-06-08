import { describe, expect, it } from "vitest";

import { FLOW_PALETTE, flowColor } from "./flow-color.js";

// Reference FNV-1a (32-bit), re-implemented so the test pins the algorithm: any
// change to the hash in flow-color.ts breaks colour stability and must fail here.
function refIndex(key: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0) % FLOW_PALETTE.length;
}

describe("flowColor", () => {
  it("has a 16-hue palette", () => {
    expect(FLOW_PALETTE).toHaveLength(16);
  });

  it("always returns a palette member", () => {
    for (const key of ["a", "a::B", "x>y", "", "🚀"]) {
      expect(FLOW_PALETTE).toContain(flowColor(key));
    }
  });

  it("is deterministic for the same key", () => {
    expect(flowColor("a::B::C")).toBe(flowColor("a::B::C"));
  });

  it("matches the reference FNV-1a hash (locks the algorithm)", () => {
    for (const key of ["a", "service>db", "Payments::Ledger", ""]) {
      expect(flowColor(key)).toBe(FLOW_PALETTE[refIndex(key)]);
    }
  });

  it("spreads distinct keys across more than one hue", () => {
    const keys = Array.from({ length: 32 }, (_, i) => `node-${i}`);
    expect(new Set(keys.map(flowColor)).size).toBeGreaterThan(1);
  });
});
