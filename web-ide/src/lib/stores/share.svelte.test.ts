import { describe, expect, it } from "vitest";

import { shareStore } from "./share.svelte.js";

describe("ShareStore", () => {
  it("toggles the busy flag", () => {
    expect(shareStore.busy).toBe(false);
    shareStore.busy = true;
    expect(shareStore.busy).toBe(true);
    shareStore.busy = false;
  });
});
