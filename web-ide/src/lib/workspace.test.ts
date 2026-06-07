import { describe, expect, it } from "vitest";

import { fqnOf, isBinaryPath, readVendoredDeps, readWorkspace } from "./workspace.js";

// A minimal in-memory File System Access tree: a plain object maps names to a
// string (file) or a nested object (directory). Implements only the subset
// `readVendoredDeps` touches (`entries`, `getDirectoryHandle`, `getFileHandle`,
// `getFile().text()`).
type Tree = { [name: string]: string | Tree };

function fakeDir(name: string, tree: Tree): FileSystemDirectoryHandle {
  const handle = {
    kind: "directory" as const,
    name,
    async *entries(): AsyncGenerator<[string, FileSystemDirectoryHandle | FileSystemFileHandle]> {
      for (const [key, value] of Object.entries(tree)) {
        yield [key, typeof value === "string" ? fakeFile(key, value) : fakeDir(key, value)];
      }
    },
    async getDirectoryHandle(child: string): Promise<FileSystemDirectoryHandle> {
      const value = tree[child];
      if (typeof value !== "object") throw new Error(`no dir ${child}`);
      return fakeDir(child, value);
    },
    async getFileHandle(child: string): Promise<FileSystemFileHandle> {
      const value = tree[child];
      if (typeof value !== "string") throw new Error(`no file ${child}`);
      return fakeFile(child, value);
    },
  };
  return handle as unknown as FileSystemDirectoryHandle;
}

function fakeFile(name: string, content: string): FileSystemFileHandle {
  return {
    kind: "file" as const,
    name,
    async getFile() {
      return { text: async () => content } as File;
    },
  } as unknown as FileSystemFileHandle;
}

describe("fqnOf", () => {
  it("derives a module FQN relative to the manifest base", () => {
    expect(fqnOf("banking/core.pds", "")).toBe("banking::core");
    expect(fqnOf("top.pds", "")).toBe("top");
    // A workspace nested under `base` strips that prefix.
    expect(fqnOf("sub/banking/core.pds", "sub")).toBe("banking::core");
  });

  it("normalises hyphens to underscores per segment (ADR-031)", () => {
    expect(fqnOf("web-ide/file-tree.pds", "")).toBe("web_ide::file_tree");
    expect(fqnOf("sub/my-mod.pds", "sub")).toBe("my_mod");
  });

  it("derives a vendored dependency's within-package FQN (base = package root)", () => {
    // readVendoredDeps reads each package relative to its own pds.toml dir, so a
    // `path = "model"` sub-workspace's `model/core.pds` becomes `core`, which the
    // resolver then prefixes with the dependency name → `dep::core`.
    expect(fqnOf("model/core.pds", "model")).toBe("core");
    expect(fqnOf("core.pds", "")).toBe("core");
  });
});

describe("readWorkspace file collection", () => {
  it("shows every folder in the tree but indexes only the consumer's own .pds", async () => {
    const root = fakeDir("proj", {
      "pds.toml": "[doc]\nname = \"app\"\n",
      "app.pds": "//! app\npublic system A;\n",
      banking: { "core.pds": "//! banking::core\npublic system Ledger;\n" },
      target: { doc: { "index.html": "<html>" }, "build.pds": "//! generated\n" },
      pds_modules: {
        "dep-abc": { "pds.toml": "[doc]\n", "x.pds": "//! x\n" },
      },
      ".git": { "HEAD": "ref: x" }, // dot-dir: hidden (tooling internal / walk hazard)
    });
    const ws = await readWorkspace(root);
    expect(ws.base).toBe("");

    // The tree is just a browser: target/, pds_modules/ (and their files) all show.
    expect(ws.dirs).toEqual(
      expect.arrayContaining(["banking", "target", "target/doc", "pds_modules", "pds_modules/dep-abc"]),
    );
    expect(ws.dirs).not.toContain(".git"); // only dot-dirs stay hidden
    const otherPaths = ws.others.map((o) => o.path);
    expect(otherPaths).toContain("target/doc/index.html");
    expect(otherPaths).toContain("target/build.pds"); // generated .pds shown, not indexed
    expect(otherPaths).toContain("pds_modules/dep-abc/x.pds");

    // The module index is scoped: only the consumer's own .pds — no vendored or
    // generated .pds, and nothing under a non-module dir.
    expect(ws.files.map((f) => f.fqn).sort()).toEqual(["app", "banking::core"]);
    expect(ws.files.some((f) => f.fqn?.includes("pds_modules") || f.path?.includes("target"))).toBe(
      false,
    );
  });
});

describe("readVendoredDeps", () => {
  it("returns the lockfile and each vendored .pds tagged with its slug + within-package FQN", async () => {
    const root = fakeDir("proj", {
      "pds.toml": "[doc]\nname = \"app\"\n",
      "pds.lock": "version = 1\n[[root]]\nname = \"banking\"\n",
      app: { "main.pds": "//! app\npublic system A;\n" },
      pds_modules: {
        "banking-0123456789ab": {
          "pds.toml": "[doc]\nname = \"banking\"\n",
          "core.pds": "//! c\npublic system Ledger;\n",
        },
      },
    });

    const { lockToml, vendored } = await readVendoredDeps(root, "");

    expect(lockToml).toContain("[[root]]");
    expect(vendored).toEqual([
      { slug: "banking-0123456789ab", fqn: "core", source: "//! c\npublic system Ledger;\n" },
    ]);
  });

  it("derives a vendored package's FQN relative to its own pds.toml (path sub-workspace)", async () => {
    const root = fakeDir("proj", {
      "pds.toml": "[doc]\nname = \"app\"\n",
      pds_modules: {
        // The package's workspace lives under `model/` (a `path = "model"` dep),
        // so `model/core.pds` must resolve to `core`, not `model::core`.
        "banking-model-0123456789ab": {
          model: {
            "pds.toml": "[doc]\nname = \"banking\"\n",
            "core.pds": "//! c\npublic system Ledger;\n",
          },
        },
      },
    });

    const { vendored } = await readVendoredDeps(root, "");
    expect(vendored).toEqual([
      { slug: "banking-model-0123456789ab", fqn: "core", source: "//! c\npublic system Ledger;\n" },
    ]);
  });

  it("returns empty when there is no pds_modules directory", async () => {
    const root = fakeDir("proj", { "pds.toml": "[doc]\nname = \"app\"\n" });
    expect(await readVendoredDeps(root, "")).toEqual({ lockToml: "", vendored: [] });
  });
});

describe("isBinaryPath", () => {
  it("flags known binary extensions, case-insensitively", () => {
    expect(isBinaryPath("logo.png")).toBe(true);
    expect(isBinaryPath("img/Photo.JPG")).toBe(true);
    expect(isBinaryPath("fonts/Inter.woff2")).toBe(true);
    expect(isBinaryPath("a/b/diagram.pdf")).toBe(true);
  });

  it("treats text and code files as non-binary", () => {
    expect(isBinaryPath("README.md")).toBe(false);
    expect(isBinaryPath("data/config.json")).toBe(false);
    expect(isBinaryPath("icon.svg")).toBe(false); // editable XML, deliberately text
    expect(isBinaryPath("main.rs")).toBe(false);
  });

  it("returns false for an extensionless path", () => {
    expect(isBinaryPath("Makefile")).toBe(false);
    expect(isBinaryPath("dir/LICENSE")).toBe(false);
  });
});
