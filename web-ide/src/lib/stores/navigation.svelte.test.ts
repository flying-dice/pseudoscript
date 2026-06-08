import { beforeEach, describe, expect, it } from "vitest";

import type { Loc } from "$lib/core/types.js";
import { navigation } from "./navigation.svelte.js";

const loc = (line: number, fileFqn = "a"): Loc => ({ fileFqn, line, col: 1 });

beforeEach(() => navigation.reset());

describe("NavigationStore", () => {
  it("records locations and tracks the cursor", () => {
    navigation.record(loc(1));
    navigation.record(loc(2));
    expect(navigation.current()).toEqual(loc(2));
    expect(navigation.index).toBe(1);
    expect(navigation.history).toHaveLength(2);
  });

  it("recordIfMoved skips a record at the same position", () => {
    navigation.record(loc(1));
    navigation.recordIfMoved(loc(1));
    expect(navigation.history).toHaveLength(1);
    navigation.recordIfMoved(loc(2));
    expect(navigation.history).toHaveLength(2);
  });

  it("steps back and forward, returning the applied Loc", () => {
    navigation.record(loc(1));
    navigation.record(loc(2));
    expect(navigation.back()).toEqual(loc(1));
    expect(navigation.index).toBe(0);
    expect(navigation.forward()).toEqual(loc(2));
    expect(navigation.index).toBe(1);
  });

  it("returns null at the boundaries without moving", () => {
    navigation.record(loc(1));
    expect(navigation.back()).toBeNull();
    expect(navigation.canBack).toBe(false);
    expect(navigation.forward()).toBeNull();
    expect(navigation.canForward).toBe(false);
  });

  it("reset clears the trail", () => {
    navigation.record(loc(1));
    navigation.reset();
    expect(navigation.history).toEqual([]);
    expect(navigation.index).toBe(-1);
  });
});
