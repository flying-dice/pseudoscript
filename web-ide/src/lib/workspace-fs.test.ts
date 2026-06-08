import { describe, expect, it } from "vitest";

import {
  createDir,
  createFile,
  deleteDir,
  deletePath,
  emptySeed,
  movePath,
  readFile,
  readWorkspace,
  sanitizeProjectName,
  scaffoldWorkspace,
  serializeManifest,
  writeFile,
} from "./workspace.js";

// A stateful in-memory File System Access tree: mutations (createWritable,
// removeEntry, getXHandle({create})) actually modify the backing nodes, so a
// write-then-read round-trips. Covers the disk-mutation primitives the IDE's
// file-tree CRUD drives (also exercised end-to-end in e2e/).
type Node = { kind: "file"; name: string; content: string } | { kind: "dir"; name: string; children: Map<string, Node> };

const dir = (name: string): Extract<Node, { kind: "dir" }> => ({ kind: "dir", name, children: new Map() });
const notFound = (n: string) => Object.assign(new Error(`NotFound: ${n}`), { name: "NotFoundError" });

function fileHandle(node: Extract<Node, { kind: "file" }>): FileSystemFileHandle {
  return {
    kind: "file",
    name: node.name,
    async getFile() {
      return { text: async () => node.content } as File;
    },
    async createWritable() {
      return {
        async write(data: unknown) {
          node.content = String(data);
        },
        async close() {},
      } as unknown as FileSystemWritableFileStream;
    },
  } as unknown as FileSystemFileHandle;
}

function dirHandle(node: Extract<Node, { kind: "dir" }>): FileSystemDirectoryHandle {
  return {
    kind: "directory",
    name: node.name,
    async *entries() {
      for (const [k, v] of node.children) yield [k, v.kind === "file" ? fileHandle(v) : dirHandle(v)];
    },
    async getDirectoryHandle(name: string, opts?: { create?: boolean }) {
      let c = node.children.get(name);
      if (!c) {
        if (!opts?.create) throw notFound(name);
        c = dir(name);
        node.children.set(name, c);
      }
      if (c.kind !== "dir") throw new Error(`not a dir: ${name}`);
      return dirHandle(c);
    },
    async getFileHandle(name: string, opts?: { create?: boolean }) {
      let c = node.children.get(name);
      if (!c) {
        if (!opts?.create) throw notFound(name);
        c = { kind: "file", name, content: "" };
        node.children.set(name, c);
      }
      if (c.kind !== "file") throw new Error(`not a file: ${name}`);
      return fileHandle(c);
    },
    async removeEntry(name: string, opts?: { recursive?: boolean }) {
      const c = node.children.get(name);
      if (!c) throw notFound(name);
      if (c.kind === "dir" && c.children.size && !opts?.recursive) throw new Error("not empty");
      node.children.delete(name);
    },
  } as unknown as FileSystemDirectoryHandle;
}

const root = () => dirHandle(dir("proj"));

describe("sanitizeProjectName", () => {
  it("lowercases, dashes spaces, strips junk and collapses dashes", () => {
    expect(sanitizeProjectName("  My  Cool App!! ")).toBe("my-cool-app");
    expect(sanitizeProjectName("--Edge.__")).toBe("edge.__".replace(/^[-.]+|[-.]+$/g, ""));
    expect(sanitizeProjectName(null)).toBe("");
  });
});

describe("emptySeed", () => {
  it("seeds a sanitized manifest and starter module", () => {
    const seed = emptySeed("My App");
    expect(seed.map((s) => s.path).sort()).toEqual(["main.pds", "pds.toml"]);
    expect(seed.find((s) => s.path === "pds.toml")!.content).toContain('name = "my-app"');
  });

  it("falls back to the default name when it sanitizes to empty", () => {
    expect(emptySeed("!!!").find((s) => s.path === "pds.toml")!.content).toContain("my-architecture");
  });
});

describe("serializeManifest", () => {
  it("regenerates the sidebar section, preserving the head tables", () => {
    const original = '[package]\nname = "x"\n\n[[doc.sidebar]]\ntitle = "Old"\nitems = []\n';
    const out = serializeManifest(original, { sidebar: [{ title: "Guide", items: [{ title: "Intro", path: "intro.md" }] }] });
    expect(out).toContain('[package]\nname = "x"');
    expect(out).not.toContain("Old");
    expect(out).toContain('title = "Guide"');
    expect(out).toContain('{ title = "Intro", path = "intro.md" },');
  });

  it("appends groups when the original has no sidebar, and returns head-only for none", () => {
    expect(serializeManifest("[doc]\n", { sidebar: [{ title: "G", items: [] }] })).toContain("[[doc.sidebar]]");
    expect(serializeManifest("[doc]\n", { sidebar: [] })).toBe("[doc]\n");
  });
});

describe("scaffoldWorkspace", () => {
  it("creates a sanitized subdir, writes the seed, and reads it back", async () => {
    const ws = await scaffoldWorkspace("My App", emptySeed("My App"), root());
    expect(ws.files.map((f) => f.fqn)).toContain("main");
    expect(ws.others.map((o) => o.path)).not.toContain("pds.toml"); // manifest, not "other"
  });
});

describe("file mutations round-trip", () => {
  it("createFile writes content readable back", async () => {
    const r = root();
    const handle = await createFile(r, "docs/intro.md", "# Hi");
    expect(await readFile(handle)).toBe("# Hi");
  });

  it("createDir persists an empty folder that readWorkspace lists", async () => {
    const r = root();
    await createDir(r, "empty/nested");
    const ws = await readWorkspace(r);
    expect(ws.dirs).toEqual(expect.arrayContaining(["empty", "empty/nested"]));
  });

  it("writeFile overwrites an existing file", async () => {
    const r = root();
    const handle = await createFile(r, "a.pds", "v1");
    await writeFile(handle, "v2");
    expect(await readFile(handle)).toBe("v2");
  });

  it("deletePath removes a file; deleteDir removes a tree", async () => {
    const r = root();
    await createFile(r, "keep.pds", "");
    await createFile(r, "drop.pds", "");
    await createFile(r, "sub/inner.pds", "");
    await deletePath(r, "drop.pds");
    await deleteDir(r, "sub");
    const ws = await readWorkspace(r);
    const paths = [...ws.files.map((f) => f.path), ...ws.others.map((o) => o.path)];
    expect(paths).toContain("keep.pds");
    expect(paths).not.toContain("drop.pds");
    expect(ws.dirs).not.toContain("sub");
  });

  it("movePath creates the destination and removes the source", async () => {
    const r = root();
    await createFile(r, "old.pds", "body");
    const moved = await movePath(r, "old.pds", "renamed.pds");
    expect(await readFile(moved)).toBe("body");
    const ws = await readWorkspace(r);
    const paths = ws.files.map((f) => f.path);
    expect(paths).toContain("renamed.pds");
    expect(paths).not.toContain("old.pds");
  });

  it("movePath uses provided contents without reading the source", async () => {
    const r = root();
    await createFile(r, "old.pds", "stale");
    const moved = await movePath(r, "old.pds", "new.pds", "fresh");
    expect(await readFile(moved)).toBe("fresh");
  });
});
