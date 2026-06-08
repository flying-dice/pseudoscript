import { beforeEach, describe, expect, it } from "vitest";

import { sessionMount } from "./session.svelte.js";

beforeEach(() => {
  sessionMount.seq = 0;
});

describe("SessionMount", () => {
  it("starts at 0 and increments on each bump", () => {
    expect(sessionMount.seq).toBe(0);
    sessionMount.bump();
    sessionMount.bump();
    expect(sessionMount.seq).toBe(2);
  });
});
