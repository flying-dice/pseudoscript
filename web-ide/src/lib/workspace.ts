// Folder-as-workspace via the File System Access API (Chromium browsers).
//
// `openWorkspace()` prompts for a directory, walks it for `.pds` modules, and
// derives each module's FQN the way the compiler does (LANG.md §8.1): relative
// to the directory that holds `pds.toml`, path separators become `::` and the
// filename is the final segment (`banking/core.pds` → `banking::core`). Files
// are read and written through their handles, so edits persist to disk.

import type { ManifestSection } from "./pds.js";

// `showDirectoryPicker` is shipped by Chromium browsers but absent from the base
// DOM lib; declare just what we use.
interface DirectoryPickerOptions {
  mode?: "read" | "readwrite";
}

declare global {
  interface Window {
    showDirectoryPicker(options?: DirectoryPickerOptions): Promise<FileSystemDirectoryHandle>;
  }
}

/** A `.pds` module discovered in a workspace. */
export interface WorkspaceFile {
  path: string;
  fqn: string;
  handle: FileSystemFileHandle;
}

/** The workspace `pds.toml`: its handle (for in-IDE edits) and base-relative path. */
export interface WorkspaceManifest {
  handle: FileSystemFileHandle;
  path: string;
}

/** A loaded workspace: its modules plus the raw `[doc]` manifest and handles. */
export interface Workspace {
  name: string;
  root: FileSystemDirectoryHandle;
  base: string;
  manifestToml: string | null;
  manifest: WorkspaceManifest | null;
  files: WorkspaceFile[];
}

/** One doc page loaded by {@link readDocPages}: a manifest item plus its content. */
export interface DocItem {
  title: string;
  path: string;
  content: string;
  handle: FileSystemFileHandle;
}

/** One sidebar group of loaded doc pages. */
export interface DocGroup {
  title: string;
  items: DocItem[];
}

/** A file opened at a path: its current text and the handle behind it. */
interface OpenedFile {
  content: string;
  handle: FileSystemFileHandle;
}

/** One generated site file written by {@link writeSite}. */
export interface SiteFile {
  path: string;
  contents: string;
}

/** Whether the host browser exposes the File System Access API. */
export const fsSupported =
  typeof window !== "undefined" && typeof window.showDirectoryPicker === "function";

const SKIP_DIRS = new Set(["node_modules", "target", ".git", ".svelte-kit"]);

/**
 * Prompts for a folder and loads it as a workspace.
 */
export async function openWorkspace(): Promise<Workspace> {
  const root = await window.showDirectoryPicker({ mode: "readwrite" });
  return readWorkspace(root);
}

/** A safe directory name: trims, lowercases spaces to hyphens, strips anything
 *  outside `[a-z0-9._-]`, and collapses repeats. Empty input yields "". */
export function sanitizeProjectName(name: string | null | undefined): string {
  return (name ?? "")
    .trim()
    .toLowerCase()
    .replace(/\s+/g, "-")
    .replace(/[^a-z0-9._-]/g, "")
    .replace(/-+/g, "-")
    .replace(/^[-.]+|[-.]+$/g, "");
}

/** The default new-project name. */
export const DEFAULT_PROJECT_NAME = "my-architecture";

/** The starter `pds.toml` for a new project (named for the workspace). */
function starterManifest(name: string): string {
  return `[package]\nname = "${name}"\n\n[doc]\nname = "${name}"\ntheme = "dark"\n`;
}

/** The starter `main.pds`: a minimal valid model that compiles clean and draws. */
function starterModule(name: string): string {
  const title = name.replace(/[-_]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  return `# The ${title} system — your architecture starts here.
system Platform {
  # A first container. Add components, data, and callables beneath it.
  container Api {
    # A first behaviour. Replace with your own flows.
    fn health() {
      # describe what happens here
    }
  }
}
`;
}

/**
 * Scaffolds a new workspace: prompts for a parent directory, creates a `<name>`
 * subdirectory under it, writes a starter `pds.toml` + `main.pds`, and returns
 * the same shape as `readWorkspace` so `mountWorkspace` just works. The name is
 * sanitized (falling back to the default if it sanitizes to empty).
 */
export async function createWorkspace(name: string): Promise<Workspace> {
  const safe = sanitizeProjectName(name) || DEFAULT_PROJECT_NAME;
  const parent = await window.showDirectoryPicker({ mode: "readwrite" });
  const root = await parent.getDirectoryHandle(safe, { create: true });

  const manifestToml = starterManifest(safe);
  const manifestHandle = await root.getFileHandle("pds.toml", { create: true });
  await writeFile(manifestHandle, manifestToml);

  const moduleHandle = await root.getFileHandle("main.pds", { create: true });
  await writeFile(moduleHandle, starterModule(safe));

  const files: WorkspaceFile[] = [{ path: "main.pds", handle: moduleHandle, fqn: "main" }];
  const manifest: WorkspaceManifest = { handle: manifestHandle, path: "pds.toml" };
  return { name: safe, root, base: "", manifestToml, manifest, files };
}

/**
 * Loads a workspace from an already-resolved directory handle (e.g. a recent
 * project's stored handle, after permission is re-granted) — the picker-free
 * half of `openWorkspace`.
 */
export async function readWorkspace(root: FileSystemDirectoryHandle): Promise<Workspace> {
  const found: Array<{ path: string; handle: FileSystemFileHandle }> = [];
  let manifestPrefix: string | null = null; // directory prefix of the shallowest pds.toml
  let manifestHandle: FileSystemFileHandle | null = null;
  await walk(root, "", found, (prefix, handle) => {
    if (manifestPrefix === null || depth(prefix) < depth(manifestPrefix)) {
      manifestPrefix = prefix;
      manifestHandle = handle;
    }
  });

  const base = manifestPrefix ?? ""; // workspace root prefix ("" = picked dir)
  const files = found
    .filter((f) => f.path.endsWith(".pds") && underBase(f.path, base))
    .map((f): WorkspaceFile => ({ path: f.path, handle: f.handle, fqn: fqnOf(f.path, base) }))
    .sort((a, b) => a.fqn.localeCompare(b.fqn));

  // The raw `[doc]` manifest, so the doc build can read `[[doc.sidebar]]` pages.
  // The handle is retained so `pds.toml` can be opened and edited in the IDE.
  const handle: FileSystemFileHandle | null = manifestHandle;
  const manifestToml = handle ? await readFile(handle) : null;
  const manifestPath = handle ? (base ? `${base}/pds.toml` : "pds.toml") : null;
  const manifest: WorkspaceManifest | null =
    handle && manifestPath ? { handle, path: manifestPath } : null;

  return { name: root.name, root, base, manifestToml, manifest, files };
}

/**
 * Reads the doc pages named by a `[[doc.sidebar]]` manifest, resolving each
 * `item.path` relative to the workspace `base` directory. `sidebar` is the
 * manifest's `sidebar` array (`[{ title, items: [{ title, path }] }]`); the
 * result mirrors it with each item's Markdown loaded into `content`. A page that
 * cannot be read is dropped (the site links only pages that exist), matching the
 * CLI's warn-and-skip.
 */
export async function readDocPages(
  root: FileSystemDirectoryHandle,
  base: string,
  sidebar: ManifestSection[] | null | undefined,
): Promise<DocGroup[]> {
  const groups: DocGroup[] = [];
  for (const group of sidebar ?? []) {
    const items: DocItem[] = [];
    for (const item of group.items ?? []) {
      const found = await openFileAt(root, base ? `${base}/${item.path}` : item.path);
      // Carry the handle so edits in the IDE editor persist to disk, like a `.pds`.
      if (found) items.push({ ...item, content: found.content, handle: found.handle });
    }
    groups.push({ title: group.title, items });
  }
  return groups;
}

/**
 * Resolves a doc-relative asset path (e.g. `./diagram.png`, `../img/x.png`) to a
 * `Blob`, walking from the workspace `root` using the open doc's path to anchor
 * the relative reference. Returns null when unresolvable (missing file, or no
 * root — a sample). Used by the Markdown live preview to render relative images.
 */
export async function resolveDocAsset(
  root: FileSystemDirectoryHandle | null | undefined,
  docPath: string,
  relPath: string,
): Promise<File | null> {
  if (!root || REMOTE_ASSET.test(relPath)) return null;
  const dir = docPath.includes("/") ? docPath.slice(0, docPath.lastIndexOf("/")) : "";
  const joined = normalizeRelPath(dir, relPath);
  if (joined == null) return null;
  const parts = joined.split("/").filter(Boolean);
  const name = parts.pop();
  if (name === undefined) return null;
  try {
    let cur = root;
    for (const part of parts) cur = await cur.getDirectoryHandle(part);
    const handle = await cur.getFileHandle(name);
    return await handle.getFile();
  } catch {
    return null;
  }
}

const REMOTE_ASSET = /^(https?:|data:|mailto:)/i;

/** Join a base dir and a relative path, resolving `.`/`..`. Returns null if it
 *  escapes above the workspace root. */
function normalizeRelPath(dir: string, rel: string): string | null {
  const stack = dir ? dir.split("/").filter(Boolean) : [];
  for (const seg of rel.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") {
      if (stack.length === 0) return null; // escapes the workspace root
      stack.pop();
    } else {
      stack.push(seg);
    }
  }
  return stack.join("/");
}

/** Opens `path` under `root`, returning `{ content, handle }`, or `null`. */
async function openFileAt(
  root: FileSystemDirectoryHandle,
  path: string,
): Promise<OpenedFile | null> {
  const parts = path.split("/");
  const name = parts.pop();
  if (name === undefined) return null;
  try {
    let dir = root;
    for (const part of parts) dir = await dir.getDirectoryHandle(part);
    const handle = await dir.getFileHandle(name);
    return { content: await readFile(handle), handle };
  } catch {
    return null;
  }
}

/** Reads a file handle's current text. */
export async function readFile(handle: FileSystemFileHandle): Promise<string> {
  const file = await handle.getFile();
  return file.text();
}

/** Overwrites a file handle with `text`. */
export async function writeFile(handle: FileSystemFileHandle, text: string): Promise<void> {
  const writable = await handle.createWritable();
  await writable.write(text);
  await writable.close();
}

/**
 * Writes generated site files under `dir` (default `target/doc`) in the
 * workspace `root`, creating intermediate directories — the CLI's `pds doc`
 * output location. `files` is `[{ path, contents }]`. Returns the output dir.
 */
export async function writeSite(
  root: FileSystemDirectoryHandle,
  files: SiteFile[],
  dir = "target/doc",
): Promise<string> {
  for (const file of files) {
    const handle = await fileHandleAt(root, `${dir}/${file.path}`);
    await writeFile(handle, file.contents);
  }
  return dir;
}

/** Resolves a writable file handle at `path` under `root`, creating dirs. */
export async function fileHandleAt(
  root: FileSystemDirectoryHandle,
  path: string,
): Promise<FileSystemFileHandle> {
  const parts = path.split("/");
  const name = parts.pop();
  if (name === undefined) throw new Error(`empty path: ${path}`);
  let dir = root;
  for (const part of parts) dir = await dir.getDirectoryHandle(part, { create: true });
  return dir.getFileHandle(name, { create: true });
}

/**
 * Resolves the parent directory handle for `path` under `root`, returning
 * `{ dir, name }` where `name` is the leaf segment. With `create`, intermediate
 * directories are created; otherwise they must already exist.
 */
export async function parentDirFor(
  root: FileSystemDirectoryHandle,
  path: string,
  { create = false }: { create?: boolean } = {},
): Promise<{ dir: FileSystemDirectoryHandle; name: string }> {
  const parts = path.split("/").filter(Boolean);
  const name = parts.pop();
  if (name === undefined) throw new Error(`empty path: ${path}`);
  let dir = root;
  for (const part of parts) dir = await dir.getDirectoryHandle(part, { create });
  return { dir, name };
}

/**
 * Creates a file at `path` under `root` (creating intermediate directories),
 * seeds it with `contents`, and returns its handle — the IDE's new-file/new-doc
 * disk primitive. Throws on a failed write so the caller can roll back any
 * in-memory change.
 */
export async function createFile(
  root: FileSystemDirectoryHandle,
  path: string,
  contents = "",
): Promise<FileSystemFileHandle> {
  const handle = await fileHandleAt(root, path);
  await writeFile(handle, contents);
  return handle;
}

/**
 * Renames or moves the file at `oldPath` to `newPath` under `root`. The FS
 * Access API has no atomic rename/move, so this creates the destination, writes
 * the (provided or read) source, then removes the source. A failed write removes
 * the half-created destination and rethrows, so neither disk nor the caller's
 * memory is left half-applied. Returns the new file handle.
 */
export async function movePath(
  root: FileSystemDirectoryHandle,
  oldPath: string,
  newPath: string,
  contents: string | null = null,
): Promise<FileSystemFileHandle> {
  const text = contents ?? (await readFile(await openHandleAt(root, oldPath)));
  const newHandle = await fileHandleAt(root, newPath);
  try {
    await writeFile(newHandle, text);
  } catch (err) {
    try {
      const { dir, name } = await parentDirFor(root, newPath);
      await dir.removeEntry(name);
    } catch {}
    throw err;
  }
  // Destination is durable; drop the source. A failure here would leave a
  // duplicate, so surface it.
  const { dir, name } = await parentDirFor(root, oldPath);
  await dir.removeEntry(name);
  return newHandle;
}

/** Deletes the file at `path` under `root`. */
export async function deletePath(root: FileSystemDirectoryHandle, path: string): Promise<void> {
  const { dir, name } = await parentDirFor(root, path);
  await dir.removeEntry(name);
}

/** Resolves an existing file handle at `path` under `root` (no create). */
async function openHandleAt(
  root: FileSystemDirectoryHandle,
  path: string,
): Promise<FileSystemFileHandle> {
  const { dir, name } = await parentDirFor(root, path);
  return dir.getFileHandle(name);
}

// ---- internals -------------------------------------------------------------

async function walk(
  dir: FileSystemDirectoryHandle,
  prefix: string,
  found: Array<{ path: string; handle: FileSystemFileHandle }>,
  onManifest: (prefix: string, handle: FileSystemFileHandle) => void,
): Promise<void> {
  for await (const [name, handle] of dir.entries()) {
    const path = prefix ? `${prefix}/${name}` : name;
    if (handle.kind === "directory") {
      if (SKIP_DIRS.has(name) || name.startsWith(".")) continue;
      await walk(handle, path, found, onManifest);
    } else {
      found.push({ path, handle });
      if (name === "pds.toml") onManifest(prefix, handle);
    }
  }
}

function depth(prefix: string): number {
  return prefix === "" ? 0 : prefix.split("/").length;
}

function underBase(path: string, base: string): boolean {
  return base === "" || path.startsWith(`${base}/`);
}

/** Path → module FQN, relative to the manifest `base` directory. */
export function fqnOf(path: string, base: string): string {
  const rel = base === "" ? path : path.slice(base.length + 1);
  return rel.replace(/\.pds$/, "").split("/").join("::");
}

/**
 * Re-serialises a `pds.toml` after a programmatic sidebar change (T10 new doc).
 * It preserves the original text up to the first `[[doc.sidebar]]` table (the
 * `[package]`/`[doc]` tables, comments, formatting) and regenerates every
 * `[[doc.sidebar]]` group from `manifest.sidebar`. With no original sidebar, the
 * regenerated groups are appended. Only the sidebar section is rebuilt — the one
 * part these flows mutate — not a general TOML round-trip.
 */
export function serializeManifest(
  originalToml: string | null | undefined,
  manifest: { sidebar?: ManifestSection[] | null },
): string {
  const text = originalToml ?? "";
  const idx = text.search(/^\[\[doc\.sidebar\]\]/m);
  const head = (idx === -1 ? text : text.slice(0, idx)).replace(/\s*$/, "\n");
  const groups = (manifest.sidebar ?? [])
    .map((g) => {
      const items = (g.items ?? [])
        .map((it) => `  { title = ${tomlStr(it.title)}, path = ${tomlStr(it.path)} },`)
        .join("\n");
      return `[[doc.sidebar]]\ntitle = ${tomlStr(g.title)}\nitems = [\n${items}\n]\n`;
    })
    .join("\n");
  return groups ? `${head}\n${groups}` : head;
}

/** A double-quoted TOML basic string (escapes `\` and `"`). */
function tomlStr(s: string | null | undefined): string {
  return `"${String(s ?? "").replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}
