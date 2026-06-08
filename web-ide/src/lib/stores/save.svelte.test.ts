import { describe, expect, it } from "vitest";

import { saveStore } from "./save.svelte.js";

describe("SaveStore", () => {
  it("defaults to an empty baseline and idle state", () => {
    expect(saveStore.persisted).toEqual({});
    expect(saveStore.saveState).toBe("idle");
  });

  it("round-trips the baseline and save state", () => {
    saveStore.persisted = { a: "system A {}" };
    saveStore.saveState = "saving";
    expect(saveStore.persisted.a).toBe("system A {}");
    expect(saveStore.saveState).toBe("saving");
    saveStore.persisted = {};
    saveStore.saveState = "idle";
  });
});
