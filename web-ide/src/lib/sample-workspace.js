// The default in-memory workspace: the canonical C4 "Internet Banking System"
// model, one module per file. Bundled at build time from the `.pds` sources via
// Vite's glob import, so it stays in sync with the files (which are also a valid
// `pds` workspace you can check/doc on disk). Editing it is session-only; use
// "Open folder" to work on a real on-disk workspace.

const sources = import.meta.glob("./sample-workspace/*.pds", {
  query: "?raw",
  import: "default",
  eager: true,
});

const files = Object.entries(sources)
  .map(([path, source]) => {
    const name = path.slice(path.lastIndexOf("/") + 1); // e.g. "api.pds"
    const fqn = name.replace(/\.pds$/, ""); // one top-level module per file
    return { path: name, fqn, source, handle: null };
  })
  .sort((a, b) => a.fqn.localeCompare(b.fqn));

/** The bundled sample workspace, shaped like an opened folder (handles null). */
export const SAMPLE_WORKSPACE = {
  name: "internet-banking · sample",
  root: null,
  files,
};
