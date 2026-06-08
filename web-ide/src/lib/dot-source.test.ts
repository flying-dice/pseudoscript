import { describe, expect, it } from "vitest";

import { type C4LayoutLike, buildC4Dot } from "./dot-source.js";

const rect = (x: number, y: number, w: number, h: number) => ({ x, y, w, h });

describe("buildC4Dot", () => {
  it("emits the header, a sized node line, and an edge", () => {
    const layout: C4LayoutLike = {
      nodes: [{ fqn: "a", label: "A", rect: rect(0, 0, 144, 72) }],
      edges: [{ from: "a", to: "a" }],
    };
    const dot = buildC4Dot(layout);
    expect(dot).toContain("digraph {");
    expect(dot).toContain("rankdir=TB;");
    expect(dot).toContain("node [shape=box, fixedsize=true];");
    expect(dot).toContain('"a" [label="A", width=2.000, height=1.000];');
    expect(dot).toContain('"a" -> "a";');
    expect(dot.trimEnd().endsWith("}")).toBe(true);
  });

  it("selects rankdir=LR", () => {
    expect(buildC4Dot({ nodes: [], edges: [] }, true)).toContain("rankdir=LR;");
  });

  it("escapes quotes and backslashes in labels", () => {
    const dot = buildC4Dot({ nodes: [{ fqn: 'a"b\\c', label: 'L"x', rect: rect(0, 0, 72, 72) }], edges: [] });
    expect(dot).toContain('"a\\"b\\\\c"');
    expect(dot).toContain('label="L\\"x"');
  });

  it("joins multiple edge labels with a newline and omits empty labels", () => {
    const dot = buildC4Dot({
      nodes: [],
      edges: [
        { from: "a", to: "b", labels: ["calls", "reads"] },
        { from: "b", to: "c", labels: [] },
      ],
    });
    expect(dot).toContain('[label="calls\nreads"]');
    expect(dot).toContain('"b" -> "c";');
    expect(dot).not.toContain('"b" -> "c" [label');
  });

  it("nests a card inside its enclosing frame as a cluster subgraph", () => {
    const dot = buildC4Dot({
      nodes: [{ fqn: "a", label: "A", rect: rect(20, 20, 40, 40) }],
      edges: [],
      boundaries: [{ fqn: "F", title: "Frame", rect: rect(0, 0, 200, 200) }],
    });
    expect(dot).toContain('subgraph "cluster_F" {');
    expect(dot).toContain('label="Frame";');
    expect(dot).toMatch(/subgraph "cluster_F" \{[\s\S]*"a" \[label="A"/);
  });

  it("nests concentric frames smallest-inside-largest", () => {
    const dot = buildC4Dot({
      nodes: [{ fqn: "a", label: "A", rect: rect(40, 40, 20, 20) }],
      edges: [],
      boundaries: [
        { fqn: "Outer", title: "Out", rect: rect(0, 0, 300, 300) },
        { fqn: "Inner", title: "In", rect: rect(20, 20, 100, 100) },
      ],
    });
    expect(dot).toMatch(/cluster_Outer[\s\S]*cluster_Inner[\s\S]*"a"/);
  });

  it("declares cards outside every frame at the top level", () => {
    const dot = buildC4Dot({
      nodes: [{ fqn: "free", label: "Free", rect: rect(500, 500, 40, 40) }],
      edges: [],
      boundaries: [{ fqn: "F", title: "Frame", rect: rect(0, 0, 100, 100) }],
    });
    // The free card is declared before the (empty) frame opens, not inside it.
    expect(dot).toMatch(/"free" \[label="Free"[\s\S]*subgraph "cluster_F"/);
  });

  it("does not adopt a card into a frame no larger than the card (area guard)", () => {
    const dot = buildC4Dot({
      nodes: [{ fqn: "a", label: "A", rect: rect(0, 0, 100, 100) }],
      edges: [],
      boundaries: [{ fqn: "F", title: "F", rect: rect(0, 0, 100, 100) }],
    });
    // Same area → not enclosed → declared at top level, before the frame.
    expect(dot).toMatch(/"a" \[label="A"[\s\S]*subgraph "cluster_F"/);
  });
});
