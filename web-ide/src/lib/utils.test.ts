import { describe, expect, it } from "vitest";

import { cn } from "./utils.js";

describe("cn", () => {
  it("resolves conflicting Tailwind utilities to the last one", () => {
    expect(cn("p-2", "p-4")).toBe("p-4");
  });

  it("drops falsy values and flattens arrays (clsx semantics)", () => {
    expect(cn("a", false && "b", ["c", null], undefined)).toBe("a c");
  });

  it("merges non-conflicting classes verbatim", () => {
    expect(cn("flex", "items-center")).toBe("flex items-center");
  });
});
