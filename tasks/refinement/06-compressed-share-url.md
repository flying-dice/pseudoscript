# T6 — Compressed share URL up to 2MB (#web-ide)

## Summary

Add a way to share an **entire multi-file workspace** (`pds.toml` + every `.pds`
module + every authored `docs/*.md` page) as a single self-contained link: serialize
the workspace to JSON, gzip-compress it, base64url-encode it, and carry it in the URL
**hash fragment** (`#w=…`). The link reconstructs the workspace client-side with no
server, no KV, no round-trip — it works offline and survives Cloudflare being down.
Target a ~2 MB practical URL ceiling, with a graceful fallback (file download/export)
when the workspace is too big to fit in a link.

**Scope correction to the task brief.** The brief assumes pre-existing share infra
(`routes/api/share/[id]/+server.js`, `routes/s/[id]`, `routes/embed/[id]`, a Share
button in the Toolbar). **None of that exists in this codebase.** See next section.
So T6 is *not* "a second share mode beside an id-based store" — it is the **first and
only** share mechanism. There is no server store to augment or fall back to.

## Current state (file:line) — existing share mechanism

There is **no share mechanism today**. Verified directly:

- `web-ide/src/routes/` contains exactly two files: `+layout.js` and `+page.svelte`.
  No `api/`, no `s/`, no `embed/` route exists.
- `web-ide/src/routes/+layout.js:1-4` — the whole app is a client-only SPA:
  `export const ssr = false; export const prerender = true;` (comment: "ideal for
  Cloudflare Pages"). So any decode logic runs in the browser, never in SSR `load`.
- `web-ide/src/lib/components/Toolbar.svelte:1-49` — the toolbar has Open-project,
  Build-docs, Settings, Format buttons and an error status. **No Share button.**
- Grep for `share` / `CompressionStream` / `gzip` / `pako` / `base64` / `btoa` /
  `location.hash` across `web-ide/src/` returns only incidental "shared" prose in
  sample docs — **zero** sharing or compression code.

The deployment target is real but unrelated to sharing:
`web-ide/package.json:13` uses `@sveltejs/adapter-cloudflare`; a `.wrangler/` dir
exists. No KV/D1/R2 binding is referenced anywhere in source.

**Single-file vs multi-file: the workspace is fully multi-file.** The model to
serialize, with exact shapes:

- `web-ide/src/lib/workspace.js:124-126` — the canonical file record:
  `WorkspaceFile = { path: string, fqn: string, handle: FileSystemFileHandle }`.
- `web-ide/src/lib/workspace.js:30-51` (`readWorkspace`) returns the workspace object
  `{ name, root, base, manifestToml, files }` — `files` is an **array** of `.pds`
  modules; `manifestToml` is the raw `pds.toml`.
- `web-ide/src/routes/+page.svelte:90-98` holds runtime state: `workspace`,
  plus `moduleSources` (FQN → live `.pds` text), `docSources` (doc path → live
  Markdown), `docMeta`, `docGroups`. **Live edited text lives in `moduleSources` /
  `docSources`, not in `workspace.files`** — the share codec MUST pull text from
  these maps, not re-read handles.
- `web-ide/src/routes/+page.svelte:137-139` (`allModules`) is the existing pattern for
  "every module as `{ fqn, source }`" — the share serializer mirrors this, plus
  `manifestToml` and the doc pages.
- Authored docs: `docGroups` / `docSources` (`+page.svelte:96-98`, populated by
  `loadWorkspaceDocs` at `:473-495`); doc pages carry `{ title, path }` and live text
  in `docSources[path]`.
- Samples are already multi-file folders: `web-ide/src/lib/samples.js:13-55` bundles
  `samples/<id>/*.pds`, `meta.json`, `pds.toml`, and `**/*.md`; `loadSample`
  (`samples.js:78-94`) materialises `{ name, root: null, files, manifestToml, docs }`
  — the same shape an opened folder produces (handles null = session-only edits).

**Current size ceiling: none.** No share path exists, so nothing bounds payload size
today. The ~2 MB target is a property of the *new* URL approach, not a migration.

**Persistence patterns to mirror:** `web-ide/src/lib/recents.js` (localStorage +
IndexedDB, all SSR-guarded and best-effort) is the house style for any client-side
persistence the feature adds.

## Proposed approach (compression, encoding, route, load-back, fallback)

**Serialization envelope (shared with T7).** One canonical, versioned JSON shape that
captures everything needed to rehydrate a session-only workspace:

```jsonc
{
  "v": 1,
  "name": "acme-tickets",
  "manifestToml": "…pds.toml…",
  "files": [ { "path": "catalog.pds", "fqn": "catalog", "text": "…" }, … ],
  "docs":  [ { "path": "docs/the-pattern.md", "title": "The pattern", "text": "…" }, … ]
}
```

Build it from `moduleSources` (live `.pds` text) + `manifestToml` + `docSources` /
`docGroups`, not from disk handles — handles don't survive a link. Sort `files` and
`docs` by path for deterministic, diff-stable links. This envelope is the single
contract reused by compressed links and **T7 import/export**.

**Compression.** Use the platform **`CompressionStream('gzip')` /
`DecompressionStream`** — baseline in modern Chromium/Firefox/Safari, zero new
dependency (no `pako` in `package.json`, and the IDE already requires the File System
Access API per `workspace.js:10-11`, so it targets modern browsers anyway).
Pipeline: `JSON.stringify(envelope)` → UTF-8 bytes → gzip → **base64url** (`+/` →
`-_`, strip `=`).

**Encoding location — hash fragment.** Put the blob in `location.hash` (`#w=…`). The
fragment is never sent to the server (privacy: the model never reaches Cloudflare
logs/CDN) and sidesteps server/CDN URL-length caps. Critical given
`+layout.js:1-4` disables SSR — the hash is only ever read in the browser anyway.

**Route / load-back.** Add a dedicated client route:

- `web-ide/src/routes/s/+page.svelte` — on `onMount`, read `location.hash`,
  base64url-decode, `DecompressionStream`-inflate, `JSON.parse`, validate `v`, then
  mount it as a session-only workspace (handles null), reusing the exact path
  `openSample` / `mountWorkspace` already take (`+page.svelte:453-506`):
  set `moduleSources` from `files`, `docSources`/`docGroups` from `docs`, then
  `mountWorkspace({ name, root: null, files, manifestToml, docs }, landing)`.
- Optionally `web-ide/src/routes/embed/+page.svelte` — same decode, minimal chrome
  for iframes (defer unless needed).

Because the app prerenders a static shell, the `/s` route is also just a prerendered
shell that decodes the hash client-side — no server code.

**Share action (new).** A `buildShareLink()` helper:

1. Assemble the envelope from current `moduleSources` / `manifestToml` / `docSources`.
2. gzip → base64url → assemble the full `/s#w=…` URL; measure its length.
3. `≤ ~2_000_000` chars → copy the link to clipboard, toast success (no network).
4. `>` threshold → toast "workspace too large for a link" and fall back to a
   **file export/download** of the same envelope (this is also T7's export path).
   No server store exists, so download is the zero-new-infra fallback.

Make the threshold a named constant. Surface semantics in the UI: a compressed link is
immutable, never expires, and embeds the **entire model** in the URL (anyone with the
link has everything, forever).

## Affected / new files

- `web-ide/src/lib/share/workspace-codec.js` **(new)** — `encodeWorkspace(envelope)` /
  `decodeWorkspace(str)`: envelope ↔ gzip+base64url, with the `v:1` schema. Shared
  with **T7**.
- `web-ide/src/lib/share/share.js` **(new)** — `buildShareLink(...)`: assemble
  envelope from the live source maps, measure, copy or fall back to export; returns
  `{ url, mode, bytes }`.
- `web-ide/src/routes/s/+page.svelte` **(new)** — decode the hash and hydrate a
  session-only workspace via `mountWorkspace`.
- `web-ide/src/routes/embed/+page.svelte` **(new, optional)** — embeddable decode view.
- `web-ide/src/lib/components/Toolbar.svelte` **(edit)** — add a Share button (new
  `onshare` prop), mirroring the existing `onbuilddocs` button at `Toolbar.svelte:29-32`.
- `web-ide/src/routes/+page.svelte` **(edit)** — wire an `onshare` handler that calls
  `buildShareLink` with `moduleSources` / `manifestToml` / `docSources`, and pass it
  to `<Toolbar>` (the Toolbar is rendered at `+page.svelte:845-853`). Factor a small
  `serializeWorkspace()` next to `allModules` (`:137-139`) for reuse by the codec.

## Open questions / decisions needed

1. **Doc-page representation in the envelope.** `docGroups` carries group/title
   structure (`+page.svelte:491-495`); decide whether to round-trip full sidebar
   grouping or just `{ path, title, text }` and let the manifest re-derive groups.
2. **`CompressionStream` only, or ship `pako`?** Recommend platform-only given the
   IDE already targets modern browsers (File System Access API).
3. **Over-threshold fallback:** export/download (no infra) vs introduce a server
   store. Recommend export-only for T6; defer any server store entirely.
4. **Dedicated `/s` route vs decoding `#w=` on the existing `/` route.** A dedicated
   route reads cleaner and makes a future `/embed` natural.
5. **Privacy/UX copy** — the full model lives in the URL forever; label it clearly.
6. **Schema versioning** — `v` is present; define unknown-version behaviour (reject
   with a clear message).

## Dependencies on other tasks (T7 import/export shares the serialization format)

- **T7 (import/export) MUST reuse the same `workspace-codec` envelope** (`v:1`, with
  `manifestToml` / `files[]` / `docs[]`). The cleanest split: land
  `workspace-codec.js` in T6 and have T7 import it; agree the schema before either
  ships or they collide on serialization. The over-threshold fallback in T6 (export a
  file) is literally T7's export path, so co-designing them is natural.
- **No branch blocker:** `web-ide/` is present and complete on this branch; T6 can
  proceed here. (The brief's pre-existing share routes simply don't exist.)

## Acceptance criteria (testable)

1. A multi-file workspace (`pds.toml` + ≥2 `.pds` + ≥1 `docs/*.md`) shared via the new
   action produces a `/s#w=…` URL with **no id** and **zero** network calls
   (assert via a network spy).
2. Opening that URL in a fresh tab reconstructs **every** file — `pds.toml`, all
   modules, all doc pages — byte-for-byte (envelope round-trip equality), and mounts a
   working session-only workspace.
3. `encodeWorkspace` output is smaller than the raw JSON for a representative text
   workspace (gzip applied — `encoded.length < json.length`).
4. A workspace whose compressed URL exceeds the threshold triggers the defined
   fallback (file export/download) and the UI reports the mode; no broken/truncated
   link is produced.
5. A workspace just under the threshold (~2 MB) still produces a link that loads back
   correctly.
6. A tampered/garbage `#w=` payload fails gracefully (clear error, no crash); an
   unknown `v` is rejected per the versioning decision.
7. Live edits are captured: editing a module then sharing reflects the edited text
   (codec reads `moduleSources` / `docSources`, not stale handles).
8. Unit test: `decodeWorkspace(encodeWorkspace(env))` deep-equals `env` — this codec
   is the contract shared with T7.

## Rough size (S/M/L) + parallel-safe?

**M.** New codec module + share helper + one client route + a Toolbar button and a
small `+page.svelte` wiring edit; no server infra. Bulk is isolated new files; the
only edits to existing files are additive (one button, one handler, one serializer
helper). **Parallel-safe with one caveat:** the `workspace-codec` envelope is a shared
contract with **T7**, so the schema must be agreed up front or the two tasks conflict
on serialization. Otherwise independent of the rest of the IDE.
