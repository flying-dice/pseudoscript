import { describe, expect, it } from "vitest";

import {
  danglingImporters,
  docPathSet,
  normalizePdsPath,
  pascalName,
  pdsSkeleton,
  slugify,
  validateNewDoc,
  validateNewFile,
  validateRename,
  withBase,
} from "./workspace-ops.js";
import type { OpenFile } from "./types.js";

describe("names & paths", () => {
  it("pascalName camel-cases and defaults to Module", () => {
    expect(pascalName("my-architecture")).toBe("MyArchitecture");
    expect(pascalName("billing_core")).toBe("BillingCore");
    expect(pascalName("")).toBe("Module");
  });

  it("normalizePdsPath trims, strips leading slash, appends .pds", () => {
    expect(normalizePdsPath("  /banking/core ")).toBe("banking/core.pds");
    expect(normalizePdsPath("a.pds")).toBe("a.pds");
    expect(normalizePdsPath("a.txt")).toBe("a.txt");
  });

  it("slugify produces a clean url slug", () => {
    expect(slugify("Release Notes!")).toBe("release-notes");
    expect(slugify("  ---  ")).toBe("");
  });

  it("withBase prefixes only when a base exists", () => {
    expect(withBase("pkg", "a.pds")).toBe("pkg/a.pds");
    expect(withBase(undefined, "a.pds")).toBe("a.pds");
  });

  it("pdsSkeleton emits a valid, drawable starter", () => {
    const s = pdsSkeleton("billing");
    expect(s).toContain("//! billing");
    expect(s).toContain("public system Billing;");
    expect(s).toContain("public container Api for Billing");
  });
});

describe("validation", () => {
  const files: OpenFile[] = [{ fqn: "a", path: "a.pds" }];

  it("validateNewFile rejects empties, backslashes, .md, pds.toml, non-pds, dupes", () => {
    expect(validateNewFile("", files, undefined)).toBe("Name can't be empty.");
    expect(validateNewFile("a\\b", files, undefined)).toBe("Use forward slashes for folders.");
    expect(validateNewFile("x/", files, undefined)).toBe("Name a file, not a folder.");
    expect(validateNewFile("notes.md", files, undefined)).toBe("Use New doc for Markdown files.");
    expect(validateNewFile("pds.toml", files, undefined)).toBe("pds.toml is reserved.");
    expect(validateNewFile("x.rs", files, undefined)).toBe("Only .pds files are supported here.");
    expect(validateNewFile("a", files, undefined)).toBe("A file with that path already exists.");
    expect(validateNewFile("b", files, undefined)).toBeNull();
  });

  it("validateRename allows the file's own path but rejects another's", () => {
    const two: OpenFile[] = [{ fqn: "a", path: "a.pds" }, { fqn: "b", path: "b.pds" }];
    const a = two[0];
    expect(validateRename(a, "a", two, undefined)).toBeNull(); // unchanged
    expect(validateRename(a, "b", two, undefined)).toBe("A file with that path already exists.");
  });

  it("validateNewDoc rejects empty/blank slug and collisions", () => {
    const paths = new Set(["docs/intro.md"]);
    expect(validateNewDoc("", paths)).toBe("Title can't be empty.");
    expect(validateNewDoc("!!!", paths)).toBe("Title needs at least one letter or number.");
    expect(validateNewDoc("Intro", paths)).toBe("A doc at docs/intro.md already exists.");
    expect(validateNewDoc("Guide", paths)).toBeNull();
  });

  it("docPathSet flattens sidebar paths", () => {
    const set = docPathSet([{ title: "G", items: [{ title: "I", path: "docs/i.md" }] }]);
    expect([...set]).toEqual(["docs/i.md"]);
  });

  it("danglingImporters finds modules still referencing the old fqn", () => {
    const files2: OpenFile[] = [{ fqn: "a" }, { fqn: "b" }];
    const src = { a: "alias X = old::Thing", b: "public system S;" };
    expect(danglingImporters(files2, src, "new", "old::Thing")).toEqual(["a"]);
  });
});
