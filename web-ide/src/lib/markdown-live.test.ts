import { describe, expect, it } from "vitest";

import { markdownLivePreview } from "./markdown-live.js";

// Smoke only: the decoration/widget geometry needs a live EditorView and real
// canvas text metrics, which jsdom can't provide — that behaviour is e2e-covered
// (fold.spec.ts and the markdown preview in ide.spec.ts). This guards that the
// extension assembles and the module loads under the test runtime.
describe("markdownLivePreview", () => {
  it("assembles a non-empty extension array", () => {
    const ext = markdownLivePreview();
    expect(Array.isArray(ext)).toBe(true);
    expect(ext.length).toBeGreaterThan(0);
  });

  it("accepts default options without throwing", () => {
    expect(() => markdownLivePreview({})).not.toThrow();
  });
});
