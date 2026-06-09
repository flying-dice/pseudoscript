import { describe, expect, it } from "vitest";

import { assembleContext } from "./fim-context.js";

describe("assembleContext", () => {
  it("splits the document at the caret", () => {
    const ctx = assembleContext("abcdef", 3);
    expect(ctx.prefix).toBe("abc");
    expect(ctx.suffix).toBe("def");
  });

  it("clamps an out-of-range caret", () => {
    expect(assembleContext("abc", -2).prefix).toBe("");
    expect(assembleContext("abc", 99).suffix).toBe("");
  });

  it("caps the prefix and suffix windows", () => {
    const doc = "a".repeat(10_000);
    const ctx = assembleContext(doc, 6000);
    expect(ctx.prefix.length).toBe(4000);
    expect(ctx.suffix.length).toBe(1500);
  });

  it("carries the grammar primer with capped in-scope symbols", () => {
    const many = Array.from({ length: 60 }, (_, i) => `mod::Node${i}`);
    const ctx = assembleContext("x", 1, many);
    expect(ctx.primer).toContain("PseudoScript");
    expect(ctx.primer).toContain("container|component Name for module::Parent");
    expect(ctx.primer).toContain("mod::Node0");
    expect(ctx.primer).toContain("mod::Node39");
    expect(ctx.primer).not.toContain("mod::Node40");
  });

  it("omits the symbol line when the workspace offers none", () => {
    expect(assembleContext("x", 0).primer).not.toContain("In-scope symbols");
  });
});
