// Folder-as-workspace via the File System Access API (Chromium browsers).
//
// `openWorkspace()` prompts for a directory, walks it for `.pds` modules, and
// derives each module's FQN the way the compiler does (LANG.md §8.1): relative
// to the directory that holds `pds.toml`, path separators become `::` and the
// filename is the final segment (`banking/core.pds` → `banking::core`). Files
// are read and written through their handles, so edits persist to disk.

/** Whether the host browser exposes the File System Access API. */
export const fsSupported =
  typeof window !== "undefined" && typeof window.showDirectoryPicker === "function";

const SKIP_DIRS = new Set(["node_modules", "target", ".git", ".svelte-kit"]);

/**
 * Prompts for a folder and loads it as a workspace.
 * @returns {Promise<{name: string, root: FileSystemDirectoryHandle, files: WorkspaceFile[]}>}
 */
export async function openWorkspace() {
  const root = await window.showDirectoryPicker({ mode: "readwrite" });
  return readWorkspace(root);
}

/**
 * Loads a workspace from an already-resolved directory handle (e.g. a recent
 * project's stored handle, after permission is re-granted) — the picker-free
 * half of `openWorkspace`.
 * @returns {Promise<{name: string, root: FileSystemDirectoryHandle, files: WorkspaceFile[]}>}
 */
export async function readWorkspace(root) {
  const found = [];
  let manifestPrefix = null; // directory prefix of the shallowest pds.toml
  let manifestHandle = null;
  await walk(root, "", found, (prefix, handle) => {
    if (manifestPrefix === null || depth(prefix) < depth(manifestPrefix)) {
      manifestPrefix = prefix;
      manifestHandle = handle;
    }
  });

  const base = manifestPrefix ?? ""; // workspace root prefix ("" = picked dir)
  const files = found
    .filter((f) => f.path.endsWith(".pds") && underBase(f.path, base))
    .map((f) => ({ path: f.path, handle: f.handle, fqn: fqnOf(f.path, base) }))
    .sort((a, b) => a.fqn.localeCompare(b.fqn));

  // The raw `[doc]` manifest, so the doc build can read `[[doc.sidebar]]` pages.
  const manifestToml = manifestHandle ? await readFile(manifestHandle) : null;

  return { name: root.name, root, base, manifestToml, files };
}

/**
 * Reads the doc pages named by a `[[doc.sidebar]]` manifest, resolving each
 * `item.path` relative to the workspace `base` directory. `sidebar` is the
 * manifest's `sidebar` array (`[{ title, items: [{ title, path }] }]`); the
 * result mirrors it with each item's Markdown loaded into `content`. A page that
 * cannot be read is dropped (the site links only pages that exist), matching the
 * CLI's warn-and-skip.
 */
export async function readDocPages(root, base, sidebar) {
  const groups = [];
  for (const group of sidebar ?? []) {
    const items = [];
    for (const item of group.items ?? []) {
      const found = await openFileAt(root, base ? `${base}/${item.path}` : item.path);
      // Carry the handle so edits in the IDE editor persist to disk, like a `.pds`.
      if (found) items.push({ ...item, content: found.content, handle: found.handle });
    }
    groups.push({ title: group.title, items });
  }
  return groups;
}

/** Opens `path` under `root`, returning `{ content, handle }`, or `null`. */
async function openFileAt(root, path) {
  const parts = path.split("/");
  const name = parts.pop();
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
export async function readFile(handle) {
  const file = await handle.getFile();
  return file.text();
}

/** Overwrites a file handle with `text`. */
export async function writeFile(handle, text) {
  const writable = await handle.createWritable();
  await writable.write(text);
  await writable.close();
}

/**
 * Writes generated site files under `dir` (default `target/doc`) in the
 * workspace `root`, creating intermediate directories — the CLI's `pds doc`
 * output location. `files` is `[{ path, contents }]`. Returns the output dir.
 */
export async function writeSite(root, files, dir = "target/doc") {
  for (const file of files) {
    const handle = await fileHandleAt(root, `${dir}/${file.path}`);
    await writeFile(handle, file.contents);
  }
  return dir;
}

/** Resolves a writable file handle at `path` under `root`, creating dirs. */
async function fileHandleAt(root, path) {
  const parts = path.split("/");
  const name = parts.pop();
  let dir = root;
  for (const part of parts) dir = await dir.getDirectoryHandle(part, { create: true });
  return dir.getFileHandle(name, { create: true });
}

/**
 * @typedef {{ path: string, fqn: string, handle: FileSystemFileHandle }} WorkspaceFile
 */

// ---- internals -------------------------------------------------------------

async function walk(dir, prefix, found, onManifest) {
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

function depth(prefix) {
  return prefix === "" ? 0 : prefix.split("/").length;
}

function underBase(path, base) {
  return base === "" || path.startsWith(`${base}/`);
}

/** Path → module FQN, relative to the manifest `base` directory. */
function fqnOf(path, base) {
  const rel = base === "" ? path : path.slice(base.length + 1);
  return rel.replace(/\.pds$/, "").split("/").join("::");
}
