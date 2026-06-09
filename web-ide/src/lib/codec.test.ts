import { Blob as NodeBlob } from "node:buffer";
import { gzipSync } from "node:zlib";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

// jsdom's Blob has no `.stream()`, which codec's CompressionStream pipeline needs.
// Node's Blob does — swap it in for this file (CompressionStream itself is a Node
// global jsdom leaves intact).
let realBlob: typeof globalThis.Blob;
beforeAll(() => {
  realBlob = globalThis.Blob;
  globalThis.Blob = NodeBlob as unknown as typeof globalThis.Blob;
});
afterAll(() => {
  globalThis.Blob = realBlob;
});

import {
  MAX_HASH_BYTES,
  base64UrlToBytes,
  buildEnvelope,
  bytesToBase64Url,
  decodeWorkspace,
  encodeWorkspace,
  envelopeToWorkspace,
} from "./codec.js";

// Node provides CompressionStream/DecompressionStream/Blob/Response, so the gzip
// round-trip runs under vitest without a browser.

const file = { path: "a.pds", fqn: "a", source: "system A {}" };

describe("buildEnvelope", () => {
  it("applies defaults for absent fields", () => {
    const env = buildEnvelope({});
    expect(env).toMatchObject({ magic: "pdsx", v: 1, name: "shared-workspace", manifestToml: null, files: [], docs: [] });
  });

  it("projects files and docs to their envelope shape, dropping extras", () => {
    const env = buildEnvelope({
      files: [{ ...file, extra: 1 } as never],
      docs: [{ path: "d.md", content: "x", extra: 2 } as never],
    });
    expect(env.files).toEqual([file]);
    expect(env.docs).toEqual([{ path: "d.md", content: "x" }]);
  });
});

describe("envelopeToWorkspace", () => {
  it("turns the docs array into a record and forces a null landing", () => {
    const ws = envelopeToWorkspace(buildEnvelope({ name: "W", docs: [{ path: "d.md", content: "x" }] }));
    expect(ws.workspace.docs).toEqual({ "d.md": "x" });
    expect(ws.landing).toBeNull();
    expect(ws.workspace.name).toBe("W");
  });
});

describe("encode/decode round-trip", () => {
  it("decodes back to the mountable workspace", async () => {
    const bytes = await encodeWorkspace({ name: "Demo", manifestToml: "[package]", files: [file], docs: [{ path: "d.md", content: "hi" }] });
    const ws = await decodeWorkspace(bytes as Uint8Array<ArrayBuffer>);
    expect(ws.workspace.name).toBe("Demo");
    expect(ws.workspace.manifestToml).toBe("[package]");
    expect(ws.workspace.files).toEqual([file]);
    expect(ws.workspace.docs).toEqual({ "d.md": "hi" });
  });
});

describe("decodeWorkspace errors", () => {
  it("rejects a corrupt/non-gzip stream", async () => {
    await expect(decodeWorkspace(new Uint8Array([1, 2, 3]))).rejects.toThrow(/corrupt or wrong format/);
  });

  it("rejects valid gzip with the wrong magic", async () => {
    await expect(decodeWorkspace(gzipJson({ magic: "nope", v: 1 }))).rejects.toThrow(/Not a PseudoScript workspace file/);
  });

  it("rejects an unsupported version", async () => {
    await expect(decodeWorkspace(gzipJson({ magic: "pdsx", v: 2 }))).rejects.toThrow(/Unsupported workspace version 2/);
  });
});

describe("base64url", () => {
  it("round-trips arbitrary bytes with URL-safe substitution", () => {
    const bytes = new Uint8Array([0xfb, 0xff, 0x3e, 0x00, 0x7f]);
    const text = bytesToBase64Url(bytes);
    expect(text).not.toMatch(/[+/=]/);
    expect([...base64UrlToBytes(text)]).toEqual([...bytes]);
  });

  it("handles empty input", () => {
    expect(bytesToBase64Url(new Uint8Array())).toBe("");
    expect(base64UrlToBytes("")).toEqual(new Uint8Array());
  });

  it("round-trips payloads larger than the 0x8000 chunk", () => {
    const big = new Uint8Array(0x8000 * 2 + 17).map((_, i) => i % 256);
    expect([...base64UrlToBytes(bytesToBase64Url(big))]).toEqual([...big]);
  });
});

it("caps the hash payload at 2 MiB", () => {
  expect(MAX_HASH_BYTES).toBe(2 * 1024 * 1024);
});

// Standard gzip via node:zlib — decodeWorkspace's DecompressionStream reads it.
// (jsdom's Blob lacks .stream(), so we avoid CompressionStream here.)
function gzipJson(obj: unknown): Uint8Array<ArrayBuffer> {
  // Copy into a fresh, zero-offset buffer — node's Buffer is a pooled view and
  // new Blob([...]) would otherwise read the wrong slice.
  return new Uint8Array(gzipSync(Buffer.from(JSON.stringify(obj)))) as Uint8Array<ArrayBuffer>;
}
