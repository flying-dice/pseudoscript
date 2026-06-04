// Canonical workspace state — a reactive rune store.
//
// Owns the open workspace, the active file, and the live buffers (modules, docs,
// manifest). The view derives `allModules`/`source`/`index` from these and runs
// the impure mutations (FS IO, mount); this store is just the owned state. Named
// `wsStore` so it doesn't clash with the many `const ws = …` locals in the view.

import type { LiveDocGroup, Module, OpenFile, WorkspaceModel } from "$lib/core/types.js";

class WorkspaceStore {
  // The live workspace: a real on-disk workspace or an in-memory sample/share.
  workspace = $state<WorkspaceModel | null>(null);
  // The active file — a module, an authored doc page, or the manifest.
  openFile = $state<OpenFile | null>(null);
  // The open editor tabs, in open order. The active one is `openFile`.
  openTabs = $state<OpenFile[]>([]);
  // In-memory module buffers, by FQN.
  moduleSources = $state<Record<string, string>>({});

  // Authored docs: sidebar groups, each page's live Markdown by path, and the
  // `{ name, theme }` parsed from `[doc]`.
  docGroups = $state<LiveDocGroup[]>([]);
  docSources = $state<Record<string, string>>({});
  docMeta = $state<{ name?: string; theme?: string }>({});

  // Companion-file buffers (non-PDS files), by base-relative path. Lazily filled
  // when a text file is opened, then edited and saved like any other buffer.
  otherSources = $state<Record<string, string>>({});

  // The raw `pds.toml` text, editable as a first-class file.
  manifestSource = $state("");
  // The last manifest parse error, shown inline when the manifest is open.
  manifestError = $state<string | null>(null);

  // Resolved direct-dependency modules (LANG.md §8.3), dependency-name-prefixed
  // (`auth::core`). Read from `pds_modules/` + `pds.lock` once on workspace mount
  // — dependencies change only via the `pds` CLI, not in-editor. The language
  // services take these as externals (public-only visibility, §8.2).
  dependencyModules = $state<Module[]>([]);

  // Every module as {fqn, source} — diagrams and diagnostics span the whole
  // workspace (cross-module edges), not just the open file.
  get allModules(): Module[] {
    return this.workspace
      ? this.workspace.files.map((f) => ({ fqn: f.fqn ?? "", source: this.moduleSources[f.fqn ?? ""] ?? "" }))
      : [];
  }

  // The dependency modules offered to the language services as externals.
  get externalModules(): Module[] {
    return this.dependencyModules;
  }
}

export const wsStore = new WorkspaceStore();
