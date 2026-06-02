// Sample-workspace catalogue, discovered from `./samples/<id>/`. Each sample is
// a self-contained folder: a `meta.json` (display name, description, the module
// to open first, sort order) plus one `.pds` file per bounded context — and a
// `pds.toml`, so the same folder also checks/docs as a real `pds` workspace on
// disk. Add a sample by dropping a folder here; nothing in this file or the page
// hardcodes a particular sample.
//
// Bundled at build time via Vite's glob so the catalogue stays in sync with the
// files. The page never autoloads one: a sample is a project *template* — the New
// project flow scaffolds its files onto disk via `sampleSeed(id)`.

import type { EnvelopeFile } from "./codec";
import type { SeedFile } from "./workspace";

/** The `meta.json` shape that sits beside each sample's `.pds` files. */
interface SampleMeta {
  name?: string;
  description?: string;
  category?: string;
  landing?: string | null;
  order?: number;
}

/** A source module within a sample template: its path, fqn, and raw text. */
export interface SampleFile extends EnvelopeFile {
  handle: FileSystemFileHandle | null;
}

/** A sample under construction, accumulated as the globs are walked. */
interface SampleDraft {
  id: string;
  meta: SampleMeta;
  files: SampleFile[];
  manifestToml: string | null;
  docs: Record<string, string>;
}

/** A catalogue entry: metadata for the examples block plus the sample's files. */
export interface Sample {
  id: string;
  name: string;
  description: string;
  category: string;
  landing: string | null;
  order: number;
  moduleCount: number;
  files: SampleFile[];
  manifestToml: string | null;
  docs: Record<string, string>;
}

const pdsSources = import.meta.glob<string>("./samples/*/*.pds", {
  query: "?raw",
  import: "default",
  eager: true,
});
const metaSources = import.meta.glob<SampleMeta>("./samples/*/meta.json", {
  import: "default",
  eager: true,
});
const tomlSources = import.meta.glob<string>("./samples/*/pds.toml", {
  query: "?raw",
  import: "default",
  eager: true,
});
const docSources = import.meta.glob<string>("./samples/*/**/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
});

// "./samples/acme-tickets/context.pds" -> "acme-tickets"
const idOf = (path: string): string => path.split("/")[2];
// "./samples/outbox/docs/the-pattern.md" -> "docs/the-pattern.md"
const relOf = (path: string): string => path.split("/").slice(3).join("/");

const byId = new Map<string, SampleDraft>();
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
export const SAMPLES: Sample[] = [...byId.values()]
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
 * A sample as scaffold seed: every `.pds` module, the `pds.toml`, and each doc
 * page, plus the page to open first. Used by the new-project flow to bootstrap
 * the example onto disk as a real workspace. Returns null for an unknown id.
 */
export function sampleSeed(id: string): { seed: SeedFile[]; landing: string | null } | null {
  const sample = SAMPLES.find((s) => s.id === id);
  if (!sample) return null;
  const seed: SeedFile[] = sample.files.map((f) => ({ path: f.path, content: f.source }));
  if (sample.manifestToml) seed.push({ path: "pds.toml", content: sample.manifestToml });
  for (const [path, content] of Object.entries(sample.docs)) seed.push({ path, content });
  return { seed, landing: sample.landing };
}

