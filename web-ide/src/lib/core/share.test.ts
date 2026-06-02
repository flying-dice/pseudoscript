import { describe, expect, it } from "vitest";

import { mountedSources, parseHashPayload, snapshotWorkspace } from "./share.js";
import type { SnapshotInput } from "./share.js";

describe("snapshotWorkspace", () => {
  it("assembles files, manifest, and docs from live state", () => {
    const input: SnapshotInput = {
      name: "demo",
      files: [{ path: "a.pds", fqn: "a" }],
      moduleSources: { a: "public system S;" },
      manifestSource: "[package]\nname='demo'",
      docGroups: [{ title: "G", items: [{ title: "Intro", path: "docs/intro.md" }] }],
      docSources: { "docs/intro.md": "# Hi" },
    };
    expect(snapshotWorkspace(input)).toEqual({
      name: "demo",
      manifestToml: "[package]\nname='demo'",
      files: [{ path: "a.pds", fqn: "a", source: "public system S;" }],
      docs: [{ path: "docs/intro.md", content: "# Hi" }],
    });
  });

  it("nulls an empty manifest and tolerates missing buffers", () => {
    const snap = snapshotWorkspace({
      name: "x",
      files: [{ fqn: "m" }],
      moduleSources: {},
      manifestSource: "",
      docGroups: [],
      docSources: {},
    });
    expect(snap.manifestToml).toBeNull();
    expect(snap.files).toEqual([{ path: "", fqn: "m", source: "" }]);
  });
});

describe("mountedSources", () => {
  it("keys decoded files by fqn", () => {
    expect(mountedSources([{ fqn: "a", source: "x" }, { fqn: "b" }])).toEqual({ a: "x", b: "" });
  });
});

describe("parseHashPayload", () => {
  it("extracts the w= payload, or null", () => {
    expect(parseHashPayload("#w=abc123")).toBe("abc123");
    expect(parseHashPayload("#foo&w=xyz&bar")).toBe("xyz");
    expect(parseHashPayload("#nothing")).toBeNull();
    expect(parseHashPayload("")).toBeNull();
  });
});
