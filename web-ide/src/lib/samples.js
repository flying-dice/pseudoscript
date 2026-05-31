// Sample-workspace catalogue, discovered from `./samples/<id>/`. Each sample is
// a self-contained folder: a `meta.json` (display name, description, the module
// to open first, sort order) plus one `.pds` file per bounded context — and a
// `pds.toml`, so the same folder also checks/docs as a real `pds` workspace on
// disk. Add a sample by dropping a folder here; nothing in this file or the page
// hardcodes a particular sample.
//
// Bundled at build time via Vite's glob so the catalogue stays in sync with the
// files. The page never autoloads one: a sample is materialised into a workspace
// only when the user picks it (the examples block / project menu), through
// `loadSample(id)`.

const pdsSources = import.meta.glob("./samples/*/*.pds", {
  query: "?raw",
  import: "default",
  eager: true,
});
const metaSources = import.meta.glob("./samples/*/meta.json", {
  import: "default",
  eager: true,
});
const tomlSources = import.meta.glob("./samples/*/pds.toml", {
  query: "?raw",
  import: "default",
  eager: true,
});
const docSources = import.meta.glob("./samples/*/**/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
});

// "./samples/acme-tickets/context.pds" -> "acme-tickets"
const idOf = (path) => path.split("/")[2];
// "./samples/outbox/docs/the-pattern.md" -> "docs/the-pattern.md"
const relOf = (path) => path.split("/").slice(3).join("/");

const byId = new Map();
for (const [path, meta] of Object.entries(metaSources)) {
  byId.set(idOf(path), { id: idOf(path), meta, files: [], manifestToml: null, docs: {} });
}
for (const [path, source] of Object.entries(pdsSources)) {
  const sample = byId.get(idOf(path));
  if (!sample) continue; // a `.pds` with no sibling meta.json is not a sample
  const file = path.slice(path.lastIndexOf("/") + 1); // "context.pds"
  sample.files.push({ path: file, fqn: file.replace(/\.pds$/, ""), source, handle: null });
}
for (const [path, source] of Object.entries(tomlSources)) {
  const sample = byId.get(idOf(path));
  if (sample) sample.manifestToml = source;
}
for (const [path, source] of Object.entries(docSources)) {
  const sample = byId.get(idOf(path));
  if (sample) sample.docs[relOf(path)] = source; // keyed by sample-relative path
}

/** The sample catalogue: metadata for the examples block, sorted for display. */
export const SAMPLES = [...byId.values()]
  .map(({ id, meta, files, manifestToml, docs }) => ({
    id,
    name: meta.name ?? id,
    description: meta.description ?? "",
    category: meta.category ?? "Examples",
    landing: meta.landing ?? null,
    order: meta.order ?? 999,
    moduleCount: files.length,
    files: [...files].sort((a, b) => a.fqn.localeCompare(b.fqn)),
    manifestToml,
    docs,
  }))
  .sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));

/**
 * Materialise a sample as an in-memory workspace (file handles null = edits are
 * session-only), shaped exactly like an opened folder, plus the module to open
 * first. Returns null for an unknown id.
 */
export function loadSample(id) {
  const sample = SAMPLES.find((s) => s.id === id);
  if (!sample) return null;
  return {
    workspace: {
      name: `${sample.name} · sample`,
      root: null,
      sampleId: sample.id,
      files: sample.files,
      // The raw `[doc]` manifest and a path→Markdown map, so the doc build
      // renders `[[doc.sidebar]]` pages the same way a real folder would.
      manifestToml: sample.manifestToml,
      docs: sample.docs,
    },
    landing: sample.landing,
  };
}
