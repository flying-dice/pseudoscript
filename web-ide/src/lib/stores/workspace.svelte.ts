// Canonical workspace state — a reactive rune store.
//
// Owns the open workspace, the active file, and the live buffers (modules, docs,
// manifest). The view derives `allModules`/`source`/`index` from these and runs
// the impure mutations (FS IO, mount); this store is just the owned state. Named
// `wsStore` so it doesn't clash with the many `const ws = …` locals in the view.

import type { LiveDocGroup, OpenFile, WorkspaceModel } from "$lib/core/types.js";

class WorkspaceStore {
  // The live workspace: a real on-disk workspace or an in-memory sample/share.
  workspace = $state<WorkspaceModel | null>(null);
  // The active file — a module, an authored doc page, or the manifest.
  openFile = $state<OpenFile | null>(null);
  // In-memory module buffers, by FQN.
  moduleSources = $state<Record<string, string>>({});

  // Authored docs: sidebar groups, each page's live Markdown by path, and the
  // `{ name, theme }` parsed from `[doc]`.
  docGroups = $state<LiveDocGroup[]>([]);
  docSources = $state<Record<string, string>>({});
  docMeta = $state<{ name?: string; theme?: string }>({});

  // The raw `pds.toml` text, editable as a first-class file.
  manifestSource = $state("");
  // The last manifest parse error, shown inline when the manifest is open.
  manifestError = $state<string | null>(null);
}

export const wsStore = new WorkspaceStore();
