import { describe, expect, it } from "vitest";

import { classifyReload, computeDirty, dirtyPaths, keyOf, seedBaseline, type Buffers } from "./dirty.js";
import type { OpenFile } from "./types.js";

const buffers = (over: Partial<Buffers> = {}): Buffers => ({
  manifestKey: "pds.toml",
  manifestSource: "",
  moduleSources: {},
  docSources: {},
  ...over,
});

describe("keyOf", () => {
  it("uses path for manifest/doc, fqn for a module", () => {
    expect(keyOf({ isManifest: true, path: "pds.toml" })).toBe("pds.toml");
    expect(keyOf({ isDoc: true, path: "docs/x.md" })).toBe("docs/x.md");
    expect(keyOf({ fqn: "m", path: "m.pds" })).toBe("m");
    expect(keyOf(null)).toBeUndefined();
  });
});

describe("computeDirty", () => {
  it("flags only buffers that diverge from their baseline", () => {
    const persisted = { m: "old", "docs/x.md": "doc", "pds.toml": "t" };
    const b = buffers({ moduleSources: { m: "new" }, docSources: { "docs/x.md": "doc" }, manifestSource: "t" });
    expect([...computeDirty(persisted, b)]).toEqual(["m"]);
  });

  it("ignores keys with no live buffer and samples with no baseline", () => {
    expect(computeDirty({}, buffers({ moduleSources: { m: "x" } })).size).toBe(0); // no baseline → never dirty
  });
});

describe("dirtyPaths", () => {
  it("maps module fqns to paths, leaves doc paths alone", () => {
    const files: OpenFile[] = [{ fqn: "m", path: "m.pds" }];
    expect([...dirtyPaths(new Set(["m", "docs/x.md"]), files)]).toEqual(["m.pds", "docs/x.md"]);
  });
});

describe("seedBaseline", () => {
  it("returns a new map with the entries advanced", () => {
    const before = { a: "1" };
    const after = seedBaseline(before, [{ key: "a", text: "2" }, { key: "b", text: "3" }]);
    expect(after).toEqual({ a: "2", b: "3" });
    expect(before).toEqual({ a: "1" }); // unmutated
  });
});

describe("classifyReload", () => {
  it("skips unchanged, reloads clean changes, flags conflicts", () => {
    expect(classifyReload("x", "x", "x")).toBe("skip");
    expect(classifyReload("disk", "base", "base")).toBe("reload");
    expect(classifyReload("disk", "base", "edited")).toBe("conflict");
  });
});
