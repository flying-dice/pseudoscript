import { describe, expect, it } from "vitest";

import { selection } from "./selection.svelte.js";

describe("SelectionStore", () => {
  it("defaults to the code view at component depth, nothing selected", () => {
    expect(selection.view).toBe("code");
    expect(selection.seqDepth).toBe("component");
    expect(selection.selected).toBeNull();
    expect(selection.pendingGoto).toBeNull();
  });

  it("round-trips field assignments", () => {
    selection.view = "canvas";
    selection.selected = { fqn: "a::B", line: 3, col: 2, fileFqn: "a" };
    selection.pendingGoto = { line: 3, col: 2, fileFqn: "a" };
    expect(selection.view).toBe("canvas");
    expect(selection.selected?.fqn).toBe("a::B");
    expect(selection.pendingGoto?.line).toBe(3);
    // restore for other tests sharing the singleton
    selection.view = "code";
    selection.selected = null;
    selection.pendingGoto = null;
  });
});
