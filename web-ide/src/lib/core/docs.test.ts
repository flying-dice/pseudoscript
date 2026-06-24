import { describe, expect, it } from "vitest";

import { buildDocConfig, findDocByPath, manifestHasDeps, removeDoc, resolveDocPath, retitleDoc, sampleDocPages } from "./docs.js";
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

describe("removeDoc", () => {
  it("drops the page at the given path", () => {
    const out = removeDoc(groups, "docs/intro.md");
    expect(out[0].items.map((i) => i.path)).toEqual(["docs/api/deep.md"]);
  });

  it("prunes a group left with no items", () => {
    const two: LiveDocGroup[] = [
      { title: "Guide", items: [{ title: "Intro", path: "docs/intro.md" }] },
      { title: "Solo", items: [{ title: "Only", path: "docs/only.md" }] },
    ];
    const out = removeDoc(two, "docs/only.md");
    expect(out.map((g) => g.title)).toEqual(["Guide"]);
  });

  it("leaves the sidebar untouched when the path is absent", () => {
    expect(removeDoc(groups, "missing.md")).toEqual(groups);
  });
});

describe("retitleDoc", () => {
  it("renames the matching page's title, keeping its path", () => {
    const out = retitleDoc(groups, "docs/intro.md", "Welcome");
    expect(out[0].items[0]).toEqual({ title: "Welcome", path: "docs/intro.md" });
    expect(out[0].items[1].title).toBe("Deep");
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
