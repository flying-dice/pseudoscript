import { beforeEach, describe, expect, it } from "vitest";

import { emptyLayoutDoc, getPins } from "$lib/core/pins.js";
import { pins } from "./pins.svelte.js";

beforeEach(() => pins.reset());

describe("PinStore", () => {
  it("defaults to an empty doc, locked, no handle", () => {
    expect(pins.doc).toEqual(emptyLayoutDoc());
    expect(pins.unlocked).toBe(false);
    expect(pins.handle).toBeNull();
  });

  it("load parses pds.layout text and stores the handle, locked", () => {
    const text = JSON.stringify({ version: 1, views: { "c4|": [{ fqn: "a", row: 0, col: 0 }] } });
    const handle = { name: "pds.layout" } as FileSystemFileHandle;
    pins.unlocked = true;
    pins.load(text, handle);
    expect(getPins(pins.doc, "c4|")).toEqual([{ fqn: "a", row: 0, col: 0 }]);
    expect(pins.handle?.name).toBe("pds.layout");
    expect(pins.unlocked).toBe(false);
  });

  it("load tolerates malformed text", () => {
    pins.load("not json", null);
    expect(pins.doc).toEqual(emptyLayoutDoc());
  });

  it("pin/freeze/unpin/clear delegate and reassign doc for reactivity", () => {
    const before = pins.doc;
    pins.pin("c4|", { fqn: "a", row: 1, col: 1 });
    expect(pins.doc).not.toBe(before);
    expect(getPins(pins.doc, "c4|")).toEqual([{ fqn: "a", row: 1, col: 1 }]);

    pins.freeze("c4|", [{ fqn: "a", row: 0, col: 0 }, { fqn: "b", row: 0, col: 1 }]);
    expect(getPins(pins.doc, "c4|")).toHaveLength(2);

    pins.unpin("c4|", "a");
    expect(getPins(pins.doc, "c4|")).toEqual([{ fqn: "b", row: 0, col: 1 }]);

    pins.clear("c4|");
    expect(getPins(pins.doc, "c4|")).toEqual([]);
  });

  it("reset forgets pins, handle and unlock", () => {
    pins.pin("c4|", { fqn: "a", row: 0, col: 0 });
    pins.reset();
    expect(pins.doc).toEqual(emptyLayoutDoc());
    expect(pins.handle).toBeNull();
    expect(pins.unlocked).toBe(false);
  });
});
