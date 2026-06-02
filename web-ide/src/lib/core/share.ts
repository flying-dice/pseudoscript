// Share / export plumbing — pure assembly around the `codec` envelope.
//
// The view owns the impure edges (gzip via codec, clipboard, file download,
// `mountWorkspace`, URL hash mutation); this module owns the value transforms:
// snapshotting the live workspace into the serialisable shape, rebuilding the
// module buffers from a decoded workspace, and extracting the hash payload.

import type { LiveDocGroup, OpenFile } from "./types.js";

// The live state a snapshot is assembled from.
export type SnapshotInput = {
  name: string;
  files: OpenFile[];
  moduleSources: Record<string, string>;
  manifestSource: string;
  docGroups: LiveDocGroup[];
  docSources: Record<string, string>;
};

// The serialisable workspace the codec envelope carries.
export type WorkspaceSnapshot = {
  name: string;
  manifestToml: string | null;
  files: { path: string; fqn: string; source: string }[];
  docs: { path: string; content: string }[];
};

/** Snapshot the live workspace (edits, manifest, docs) into the codec shape. */
export function snapshotWorkspace(input: SnapshotInput): WorkspaceSnapshot {
  const files = input.files.map((f) => ({
    path: f.path ?? "",
    fqn: f.fqn ?? "",
    source: input.moduleSources[f.fqn ?? ""] ?? "",
  }));
  const docs: { path: string; content: string }[] = [];
  for (const g of input.docGroups) for (const it of g.items) docs.push({ path: it.path, content: input.docSources[it.path] ?? "" });
  return { name: input.name, manifestToml: input.manifestSource || null, files, docs };
}

/** Rebuild the in-memory module buffers from a decoded workspace's files. */
export function mountedSources(files: { fqn?: string; source?: string }[]): Record<string, string> {
  return Object.fromEntries(files.map((f) => [f.fqn ?? "", f.source ?? ""]));
}

/** The base64url `w=` payload in a URL hash, or null when absent. */
export function parseHashPayload(hash: string): string | null {
  return hash.match(/[#&]w=([^&]+)/)?.[1] ?? null;
}
