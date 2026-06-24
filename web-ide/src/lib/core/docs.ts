// Manifest / doc-sidebar resolution — pure.
//
// The async page loaders (reading `.md` from disk, the manifest parse, the
// load-race guard) stay in the workspace store; this module owns the value
// transforms: link path normalisation, sidebar lookup, the doc-site config, the
// sample-page fold, and the `[dependencies]` probe. No Svelte, no FS.

import type { LiveDocGroup, LiveDocItem } from "./types.js";

/** Whether the manifest declares a `[dependencies]` table. */
export function manifestHasDeps(toml: string): boolean {
  return /^\s*\[dependencies/m.test(toml);
}

/** Normalise a relative doc link against the open doc's directory to a workspace
 *  path (resolving `.`/`..`, stripping any `#`/`?` suffix). */
export function resolveDocPath(openDocPath: string, rel: string): string {
  const dir = openDocPath.includes("/") ? openDocPath.slice(0, openDocPath.lastIndexOf("/")) : "";
  const stack = dir ? dir.split("/").filter(Boolean) : [];
  for (const seg of rel.replace(/[#?].*$/, "").split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") stack.pop();
    else stack.push(seg);
  }
  return stack.join("/");
}

/** The sidebar item at `path`, or undefined. */
export function findDocByPath(docGroups: LiveDocGroup[], path: string): LiveDocItem | undefined {
  return docGroups.flatMap((g) => g.items).find((it) => it.path === path);
}

/** Drop the doc at `path` from the sidebar, pruning any group left with no items
 *  (an authored group whose last page is deleted disappears, not lingers empty). */
export function removeDoc(docGroups: LiveDocGroup[], path: string): LiveDocGroup[] {
  return docGroups
    .map((g) => ({ ...g, items: g.items.filter((it) => it.path !== path) }))
    .filter((g) => g.items.length > 0);
}

/** Retitle the doc at `path`; its path (and on-disk file) are unchanged. */
export function retitleDoc(docGroups: LiveDocGroup[], path: string, title: string): LiveDocGroup[] {
  return docGroups.map((g) => ({ ...g, items: g.items.map((it) => (it.path === path ? { ...it, title } : it)) }));
}

// The doc-site config the renderer consumes (a structural subset of pds.ts's
// `DocConfig` — typed here so core doesn't depend on the WASM module).
export type DocConfigInput = {
  name: string;
  theme: string;
  docGroups: LiveDocGroup[];
  docSources: Record<string, string>;
};
export type DocSiteConfig = {
  name: string;
  theme: string;
  docs: { title: string; items: { title: string; path: string; content: string }[] }[];
};

/** Assemble the doc-site render config from the live manifest meta + pages. */
export function buildDocConfig(input: DocConfigInput): DocSiteConfig {
  return {
    name: input.name,
    theme: input.theme,
    docs: input.docGroups.map((g) => ({
      title: g.title,
      items: g.items.map((i) => ({ title: i.title, path: i.path, content: input.docSources[i.path] ?? "" })),
    })),
  };
}

/** Fold bundled sample Markdown into the manifest sidebar, dropping any page with
 *  no bundled content (mirrors the folder path's warn-and-skip). */
export function sampleDocPages(
  sidebar: { title: string; items?: { title: string; path: string }[] }[] | null | undefined,
  docMap: Record<string, string>,
): { title: string; items: { title: string; path: string; content: string }[] }[] {
  return (sidebar ?? []).map((group) => ({
    title: group.title,
    items: (group.items ?? []).filter((item) => docMap[item.path] != null).map((item) => ({ ...item, content: docMap[item.path] })),
  }));
}
