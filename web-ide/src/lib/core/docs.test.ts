import { describe, expect, it } from "vitest";

import { buildDocConfig, findDocByPath, manifestHasDeps, resolveDocPath, sampleDocPages } from "./docs.js";
import type { LiveDocGroup } from "./types.js";

const groups: LiveDocGroup[] = [
  { title: "Guide", items: [{ title: "Intro", path: "docs/intro.md" }, { title: "Deep", path: "docs/api/deep.md" }] },
];

describe("manifestHasDeps", () => {
  it("detects a [dependencies] table", () => {
    expect(manifestHasDeps("[package]\nname='x'\n[dependencies]\n")).toBe(true);
    expect(manifestHasDeps("  [dependencies.foo]")).toBe(true);
    expect(manifestHasDeps("[package]\nname='x'")).toBe(false);
  });
});

describe("resolveDocPath", () => {
  it("resolves siblings, ., and .. against the open doc's dir", () => {
    expect(resolveDocPath("docs/intro.md", "api/deep.md")).toBe("docs/api/deep.md");
    expect(resolveDocPath("docs/api/deep.md", "../intro.md")).toBe("docs/intro.md");
    expect(resolveDocPath("docs/intro.md", "./intro.md#anchor")).toBe("docs/intro.md");
    expect(resolveDocPath("intro.md", "other.md")).toBe("other.md");
  });
});

describe("findDocByPath", () => {
  it("finds a sidebar item by path", () => {
    expect(findDocByPath(groups, "docs/api/deep.md")?.title).toBe("Deep");
    expect(findDocByPath(groups, "missing.md")).toBeUndefined();
  });
});

describe("buildDocConfig", () => {
  it("inlines page content under each group", () => {
    const cfg = buildDocConfig({ name: "X", theme: "dark", docGroups: groups, docSources: { "docs/intro.md": "# Hi" } });
    expect(cfg.name).toBe("X");
    expect(cfg.docs[0].items[0]).toEqual({ title: "Intro", path: "docs/intro.md", content: "# Hi" });
    expect(cfg.docs[0].items[1].content).toBe(""); // missing buffer → empty
  });
});

describe("sampleDocPages", () => {
  it("folds bundled content and drops pages with none", () => {
    const sidebar = [{ title: "G", items: [{ title: "A", path: "a.md" }, { title: "B", path: "b.md" }] }];
    const out = sampleDocPages(sidebar, { "a.md": "content" });
    expect(out[0].items).toEqual([{ title: "A", path: "a.md", content: "content" }]);
  });
});
