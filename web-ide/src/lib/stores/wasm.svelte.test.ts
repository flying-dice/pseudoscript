import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { initWasm, version } = vi.hoisted(() => ({
  initWasm: vi.fn(),
  version: vi.fn(() => "1.2.3"),
}));
vi.mock("$lib/pds.js", () => ({ initWasm, version }));

import { wasm } from "./wasm.svelte.js";

beforeEach(() => {
  wasm.ready = false;
  wasm.error = null;
  wasm.version = "";
  initWasm.mockReset().mockResolvedValue(undefined);
  version.mockReset().mockReturnValue("1.2.3");
});
afterEach(() => vi.clearAllMocks());

describe("WasmStore.init", () => {
  it("resolves true and reads the version on success, leaving ready for the caller", async () => {
    expect(await wasm.init()).toBe(true);
    expect(wasm.version).toBe("1.2.3");
    expect(wasm.error).toBeNull();
    expect(wasm.ready).toBe(false); // the caller flips ready after boot
  });

  it("resolves false and records the error on failure", async () => {
    initWasm.mockRejectedValueOnce(new Error("no wasm"));
    expect(await wasm.init()).toBe(false);
    expect(wasm.error).toBe("no wasm");
    expect(wasm.version).toBe(""); // untouched
  });

  it("clears a prior error at the start of init", async () => {
    wasm.error = "stale";
    await wasm.init();
    expect(wasm.error).toBeNull();
  });
});
