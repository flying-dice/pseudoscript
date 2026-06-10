// The three-way folder-pick classification (model: ide::PickError — cancel and
// failure are never conflated). `fsSupported` is computed at module load, so
// each case stubs the picker *before* a fresh import.
import { afterEach, describe, expect, it, vi } from "vitest";

const importFresh = async () => {
  vi.resetModules();
  return import("./workspace.js");
};

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("pickDirectoryOutcome", () => {
  it("fails with a browser hint when the File System Access API is missing", async () => {
    const { pickDirectoryOutcome } = await importFresh();
    const outcome = await pickDirectoryOutcome();
    expect(outcome.kind).toBe("failed");
    if (outcome.kind === "failed") expect(outcome.message).toMatch(/Chrome, Edge, Brave, or Arc/);
  });

  it("classifies an AbortError as a silent cancel", async () => {
    vi.stubGlobal(
      "showDirectoryPicker",
      vi.fn(() => Promise.reject(new DOMException("user dismissed", "AbortError"))),
    );
    const { pickDirectoryOutcome } = await importFresh();
    expect(await pickDirectoryOutcome()).toEqual({ kind: "cancelled" });
  });

  it("classifies any other rejection as a failure carrying its cause", async () => {
    vi.stubGlobal(
      "showDirectoryPicker",
      vi.fn(() => Promise.reject(new DOMException("blocked by permissions policy", "SecurityError"))),
    );
    const { pickDirectoryOutcome } = await importFresh();
    const outcome = await pickDirectoryOutcome();
    expect(outcome).toEqual({ kind: "failed", message: "blocked by permissions policy" });
  });

  it("returns the handle on a successful pick", async () => {
    const handle = { name: "workspace" } as FileSystemDirectoryHandle;
    vi.stubGlobal(
      "showDirectoryPicker",
      vi.fn(() => Promise.resolve(handle)),
    );
    const { pickDirectoryOutcome } = await importFresh();
    expect(await pickDirectoryOutcome()).toEqual({ kind: "picked", handle });
  });
});
