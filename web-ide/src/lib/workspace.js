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

  const found = [];
  let manifestPrefix = null; // directory prefix of the shallowest pds.toml
  await walk(root, "", found, (prefix) => {
    if (manifestPrefix === null || depth(prefix) < depth(manifestPrefix)) {
      manifestPrefix = prefix;
    }
  });

  const base = manifestPrefix ?? ""; // workspace root prefix ("" = picked dir)
  const files = found
    .filter((f) => f.path.endsWith(".pds") && underBase(f.path, base))
    .map((f) => ({ path: f.path, handle: f.handle, fqn: fqnOf(f.path, base) }))
    .sort((a, b) => a.fqn.localeCompare(b.fqn));

  return { name: root.name, root, files };
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
      if (name === "pds.toml") onManifest(prefix);
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
