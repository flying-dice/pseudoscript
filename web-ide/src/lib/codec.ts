// Workspace serialisation — one versioned, gzipped JSON envelope, two transports:
// T6 base64url's the bytes into the URL hash; T7 downloads them as a `.pdsx` file.
// Compression uses the platform `CompressionStream` (Chromium/modern browsers),
// the same engine the rest of the IDE assumes (File System Access is Chromium-only
// too), so there's no extra dependency.
//
// Envelope shape (v1): a plain in-memory workspace, sans FS handles —
//   { v: 1, name, manifestToml, files: [{ path, fqn, source }],
//     docs: [{ path, content }] }
// which is exactly what `mountWorkspace` consumes after `loadSample`.

/** A single source module within a workspace envelope. */
export interface EnvelopeFile {
  path: string;
  fqn: string;
  source: string;
}

/** A single doc page within a workspace envelope. */
export interface EnvelopeDoc {
  path: string;
  content: string;
}

/** The versioned, serialisable envelope written to the hash/`.pdsx` file. */
export interface Envelope {
  magic: string;
  v: number;
  name: string;
  manifestToml: string | null;
  files: EnvelopeFile[];
  docs: EnvelopeDoc[];
}

/** The live workspace snapshot the IDE feeds into the codec. */
export interface WorkspaceState {
  name?: string | null;
  manifestToml?: string | null;
  files?: EnvelopeFile[] | null;
  docs?: EnvelopeDoc[] | null;
}

/** The in-memory workspace shape (`{ workspace, landing }`) `mountWorkspace`
 *  consumes — identical to what `loadSample` returns. */
export interface MountableWorkspace {
  workspace: {
    name: string;
    files: EnvelopeFile[];
    manifestToml: string | null;
    docs: Record<string, string>;
  };
  landing: string | null;
}

const ENVELOPE_VERSION = 1;
const MAGIC = "pdsx"; // leading tag so a wrong file fails fast with a clear error

/** Build the versioned envelope from the IDE's live workspace state. */
export function buildEnvelope({ name, manifestToml, files, docs }: WorkspaceState): Envelope {
  return {
    magic: MAGIC,
    v: ENVELOPE_VERSION,
    name: name ?? "shared-workspace",
    manifestToml: manifestToml ?? null,
    files: (files ?? []).map((f) => ({ path: f.path, fqn: f.fqn, source: f.source })),
    docs: (docs ?? []).map((d) => ({ path: d.path, content: d.content })),
  };
}

/** Restore the in-memory workspace shape (`{ workspace, landing }`) from an
 *  envelope — the same shape `loadSample` returns, so mounting is identical. */
export function envelopeToWorkspace(env: Envelope): MountableWorkspace {
  const docs: Record<string, string> = {};
  for (const d of env.docs ?? []) docs[d.path] = d.content;
  return {
    workspace: {
      name: env.name ?? "shared-workspace",
      files: (env.files ?? []).map((f) => ({ path: f.path, fqn: f.fqn, source: f.source })),
      manifestToml: env.manifestToml ?? null,
      docs,
    },
    // No explicit landing: the IDE applies its doc-default rule (first doc page,
    // else first module) on mount.
    landing: null,
  };
}

// ---- gzip via CompressionStream -------------------------------------------

async function gzip(bytes: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
  const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function gunzip(bytes: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/** Encode a workspace state object to gzipped envelope bytes. */
export async function encodeWorkspace(state: WorkspaceState): Promise<Uint8Array> {
  const json = JSON.stringify(buildEnvelope(state));
  return gzip(new TextEncoder().encode(json));
}

/** Decode gzipped envelope bytes back to `{ workspace, landing }`. Throws on a
 *  corrupt stream or a wrong/unsupported envelope. */
export async function decodeWorkspace(bytes: Uint8Array<ArrayBuffer>): Promise<MountableWorkspace> {
  let env: Envelope;
  try {
    const json = new TextDecoder().decode(await gunzip(bytes));
    env = JSON.parse(json) as Envelope;
  } catch {
    throw new Error("Not a valid PseudoScript workspace (corrupt or wrong format).");
  }
  if (env?.magic !== MAGIC) throw new Error("Not a PseudoScript workspace file.");
  if (env.v !== ENVELOPE_VERSION) throw new Error(`Unsupported workspace version ${env.v}.`);
  return envelopeToWorkspace(env);
}

// ---- base64url (for the URL hash, T6) -------------------------------------

/** Bytes → URL-safe base64 (no padding), chunked to avoid call-stack limits. */
export function bytesToBase64Url(bytes: Uint8Array): string {
  let bin = "";
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** URL-safe base64 → bytes. */
export function base64UrlToBytes(text: string): Uint8Array {
  const b64 = text.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

/** The maximum encoded payload (bytes) to put in a URL hash before falling back
 *  to a file export. URLs over a couple MB are unreliable across browsers. */
export const MAX_HASH_BYTES = 2 * 1024 * 1024;
