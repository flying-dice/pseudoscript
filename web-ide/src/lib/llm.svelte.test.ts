import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LLM_DEFAULTS, llm } from "./llm.svelte.js";

// The store is a module singleton; reset it (and storage) before each test.
beforeEach(() => {
  localStorage.clear();
  llm.reset();
});
afterEach(() => localStorage.clear());

describe("defaults", () => {
  it("starts disabled with the local-Ollama placeholders", () => {
    expect(llm.enabled).toBe(false);
    expect(llm.baseUrl).toBe(LLM_DEFAULTS.baseUrl);
    expect(llm.mode).toBe("chat");
    expect(llm.ready).toBe(false);
  });
});

describe("set / ready / version", () => {
  it("applies a partial patch and persists it", () => {
    llm.set({ enabled: true, apiKey: "sk-test" });
    expect(llm.enabled).toBe(true);
    expect(llm.apiKey).toBe("sk-test");
    expect(llm.model).toBe(LLM_DEFAULTS.model); // untouched fields keep defaults
    expect(JSON.parse(localStorage.getItem("pds.llm")!)).toMatchObject({
      enabled: true,
      apiKey: "sk-test",
    });
  });

  it("is ready only when enabled with an endpoint and a model", () => {
    llm.set({ enabled: true });
    expect(llm.ready).toBe(true);
    llm.set({ baseUrl: "  " });
    expect(llm.ready).toBe(false);
    llm.set({ baseUrl: "https://api.example.test/v1", model: "" });
    expect(llm.ready).toBe(false);
  });

  it("bumps version on every change", () => {
    const v = llm.version;
    llm.set({ model: "codestral-latest" });
    expect(llm.version).toBeGreaterThan(v);
  });

  it("snapshot returns a detached copy", () => {
    const snap = llm.snapshot();
    llm.set({ model: "other" });
    expect(snap.model).toBe(LLM_DEFAULTS.model);
  });

  it("reset returns to defaults and clears storage", () => {
    llm.set({ enabled: true, apiKey: "sk-test" });
    llm.reset();
    expect(llm.enabled).toBe(false);
    expect(llm.apiKey).toBe("");
    expect(localStorage.getItem("pds.llm")).toBeNull();
  });
});

describe("persistence (fresh module load)", () => {
  it("restores persisted settings, coercing bad fields to defaults", async () => {
    localStorage.setItem(
      "pds.llm",
      JSON.stringify({ enabled: true, baseUrl: "https://api.x.test/v1", mode: "bogus", model: 7 }),
    );
    vi.resetModules();
    const mod = await import("./llm.svelte.js");
    expect(mod.llm.enabled).toBe(true);
    expect(mod.llm.baseUrl).toBe("https://api.x.test/v1");
    expect(mod.llm.mode).toBe("chat"); // unknown mode dropped
    expect(mod.llm.model).toBe(mod.LLM_DEFAULTS.model); // wrong type dropped
  });

  it("survives unparseable storage", async () => {
    localStorage.setItem("pds.llm", "{nope");
    vi.resetModules();
    const mod = await import("./llm.svelte.js");
    expect(mod.llm.enabled).toBe(false);
  });
});
