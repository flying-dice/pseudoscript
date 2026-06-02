// Dirty-state and baseline math — pure.
//
// "Dirty" = a live buffer differs from its on-disk baseline. Only files with a
// recorded baseline (handle-backed) can be dirty; in-memory samples have none.
// The debounce/FS/timer half of saving lives in the save store — this is only
// the derivation and the reload decision. No Svelte, no FS.

import type { OpenFile } from "./types.js";

// The live buffers, by key, that a baseline is compared against.
export type Buffers = {
  manifestKey: string | null;
  manifestSource: string;
  moduleSources: Record<string, string>;
  docSources: Record<string, string>;
  otherSources: Record<string, string>;
};

/** A file's buffer key: its path for a doc, manifest, or companion file; its FQN
 *  for a module. */
export function keyOf(file: OpenFile | null): string | undefined {
  return file?.isManifest || file?.isDoc || file?.isOther ? file.path : file?.fqn;
}

/** The live value for a baseline key, across the manifest / module / doc / companion buffers. */
function currentFor(key: string, b: Buffers): string | undefined {
  if (key === b.manifestKey) return b.manifestSource;
  if (key in b.moduleSources) return b.moduleSources[key];
  if (key in b.docSources) return b.docSources[key];
  return b.otherSources[key];
}

/** The set of dirty keys: every baseline whose live buffer diverged from it. */
export function computeDirty(persisted: Record<string, string>, buffers: Buffers): Set<string> {
  const set = new Set<string>();
  for (const key of Object.keys(persisted)) {
    const current = currentFor(key, buffers);
    if (current !== undefined && current !== persisted[key]) set.add(key);
  }
  return set;
}

/** Dirty keys mapped to tree paths for the file-tree dot: module keys are FQNs
 *  (resolved to their path); doc/manifest keys are already paths. */
export function dirtyPaths(dirty: Set<string>, files: OpenFile[]): Set<string> {
  const pathByFqn = new Map<string, string>();
  for (const f of files) if (f.fqn && f.path) pathByFqn.set(f.fqn, f.path);
  const paths = new Set<string>();
  for (const key of dirty) paths.add(pathByFqn.get(key) ?? key);
  return paths;
}

/** Advance the baseline for a batch of saved buffers; returns a new map. */
export function seedBaseline(
  persisted: Record<string, string>,
  entries: { key: string; text: string }[],
): Record<string, string> {
  const next = { ...persisted };
  for (const { key, text } of entries) next[key] = text;
  return next;
}

/**
 * Decide how to handle a file whose disk content was re-read. `base` is its
 * baseline, `buffer` the live buffer:
 *   - `skip`    — unchanged on disk (`disk === base`).
 *   - `conflict`— disk changed but the buffer has unsaved edits (`buffer !== base`).
 *   - `reload`  — disk changed and the buffer is clean; safe to pull in.
 */
export function classifyReload(disk: string, base: string, buffer: string | undefined): "skip" | "reload" | "conflict" {
  if (disk === base) return "skip";
  if (buffer !== base) return "conflict";
  return "reload";
}
