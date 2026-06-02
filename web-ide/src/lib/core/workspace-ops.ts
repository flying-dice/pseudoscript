// Workspace naming, path, and validation helpers — pure.
//
// The view passes in the current file set / base / doc paths rather than these
// reading global state, so each is a referentially-transparent function the unit
// tests can pin. No Svelte, no FS.

import type { LiveDocGroup, OpenFile } from "./types.js";

/** A kebab/underscore name as a PascalCase identifier, defaulting to `Module`. */
export function pascalName(s: string): string {
  return (
    s
      .replace(/[-_]+/g, " ")
      .replace(/\b\w/g, (c) => c.toUpperCase())
      .replace(/\s+/g, "") || "Module"
  );
}

/** The starter `.pds` module for a new file: a minimal valid, drawable model. */
export function pdsSkeleton(fqn: string): string {
  const leaf = fqn.split("::").pop() ?? "";
  const title = leaf.replace(/[-_]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  const sys = pascalName(leaf);
  return `//! ${leaf}

/// The ${title} system — describe this module's architecture here.
public system ${sys};

/// A first container. Add components, data, and callables beneath it.
public container Api for ${sys} {
  /// A first behaviour. Replace with your own flows.
  public Health(): void {
    // Describe what happens here.
  }
}
`;
}

/** Normalise a typed name into a base-relative leaf path: trim, drop a leading
 *  slash, append `.pds` when no extension is given. */
export function normalizePdsPath(name: string): string {
  let p = name.trim().replace(/^\/+/, "");
  if (!/\.[a-z0-9]+$/i.test(p)) p += ".pds";
  return p;
}

/** Normalise a typed folder path: trim each segment, drop empties and a stray
 *  `.pds`, and join with `/`. `"  banking//adapters/ "` → `"banking/adapters"`. */
export function normalizeDirPath(name: string): string {
  return name
    .trim()
    .replace(/\\/g, "/")
    .replace(/\.pds$/i, "")
    .split("/")
    .map((s) => s.trim())
    .filter(Boolean)
    .join("/");
}

/** True when renaming folder `oldRel` to `newRel` is illegal because the target
 *  is the folder itself or nested in/around it — a move into your own subtree
 *  would delete what you just moved. */
export function folderRenameClash(oldRel: string, newRel: string): boolean {
  return newRel === oldRel || newRel.startsWith(`${oldRel}/`) || oldRel.startsWith(`${newRel}/`);
}

/** Re-point a directory list when folder `oldRel` is renamed to `newRel`: the
 *  folder and everything under it move; the new path is guaranteed present. */
export function remapDirs(dirs: string[], oldRel: string, newRel: string): string[] {
  const prefix = `${oldRel}/`;
  const mapped = dirs.map((d) => (d === oldRel ? newRel : d.startsWith(prefix) ? `${newRel}${d.slice(oldRel.length)}` : d));
  return Array.from(new Set([...mapped, newRel])).sort();
}

/** A doc title as a URL slug. */
export function slugify(title: string): string {
  return title
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/** Prefix a base-relative path with the workspace base dir (the manifest dir). */
export function withBase(base: string | undefined, rel: string): string {
  return base ? `${base}/${rel}` : rel;
}

/** Every doc path already in the live sidebar (base-relative). */
export function docPathSet(docGroups: LiveDocGroup[]): Set<string> {
  const set = new Set<string>();
  for (const g of docGroups) for (const it of g.items) set.add(it.path);
  return set;
}

/** Validate a new `.pds` path against the file set + reserved names. `/` is a
 *  directory separator; `\` and empty are rejected. Returns an error or null. */
export function validateNewFile(name: string, files: OpenFile[], base: string | undefined): string | null {
  const raw = name.trim();
  if (!raw) return "Name can't be empty.";
  if (raw.includes("\\")) return "Use forward slashes for folders.";
  if (raw.endsWith("/")) return "Name a file, not a folder.";
  const lower = raw.toLowerCase();
  if (lower.endsWith(".md")) return "Use New doc for Markdown files.";
  if (/(^|\/)pds\.toml$/i.test(raw)) return "pds.toml is reserved.";
  if (/\.[a-z0-9]+$/i.test(raw) && !lower.endsWith(".pds")) return "Only .pds files are supported here.";
  const path = withBase(base, normalizePdsPath(raw));
  if (files.some((f) => f.path === path)) return "A file with that path already exists.";
  return null;
}

/** Validate a rename target: like {@link validateNewFile}, but the file's own
 *  current path / unchanged name does not count as a collision. */
export function validateRename(
  file: OpenFile,
  name: string,
  files: OpenFile[],
  base: string | undefined,
): string | null {
  const err = validateNewFile(name, files, base);
  if (!err) return null;
  if (err === "A file with that path already exists.") {
    const target = withBase(base, normalizePdsPath(name));
    if (!files.some((f) => f !== file && f.path === target)) return null;
  }
  return err;
}

/** Validate a new doc title against the existing sidebar paths. */
export function validateNewDoc(title: string, docPaths: Set<string>): string | null {
  const t = title.trim();
  if (!t) return "Title can't be empty.";
  const slug = slugify(t);
  if (!slug) return "Title needs at least one letter or number.";
  if (docPaths.has(`docs/${slug}.md`)) return `A doc at docs/${slug}.md already exists.`;
  return null;
}

/** Modules whose source still references `oldFqn` after a rename — a warn-only
 *  signal (importers are not auto-rewritten). A verbatim substring test suffices. */
export function danglingImporters(
  files: OpenFile[],
  moduleSources: Record<string, string>,
  newFqn: string,
  oldFqn: string,
): string[] {
  return files
    .filter((f) => f.fqn !== newFqn && (moduleSources[f.fqn ?? ""] ?? "").includes(oldFqn))
    .map((f) => f.fqn ?? "");
}
