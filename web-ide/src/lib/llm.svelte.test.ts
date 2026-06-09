import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ProviderError } from "./fim-provider.js";
import { LLM_DEFAULTS, PRESETS, llm, usableModels } from "./llm.svelte.js";

// The store is a module singleton; reset it (and storage) before each test.
beforeEach(() => {
  localStorage.clear();
  llm.reset();
});
afterEach(() => localStorage.clear());

describe("defaults", () => {
  it("starts disabled on the Ollama preset", () => {
    expect(llm.enabled).toBe(false);
    expect(llm.provider).toBe("ollama");
    expect(llm.baseUrl).toBe(LLM_DEFAULTS.baseUrl);
    expect(llm.mode).toBe("chat");
    expect(llm.ready).toBe(false);
  });
});

describe("presets", () => {
  it("applyPreset pins the provider's endpoint, model, and mode", () => {
    llm.applyPreset("openai");
    expect(llm.provider).toBe("openai");
    expect(llm.baseUrl).toBe(PRESETS.openai.baseUrl);
    expect(llm.model).toBe(PRESETS.openai.model);
    expect(llm.mode).toBe("chat");
  });

  it("switching presets keeps the API key", () => {
    llm.applyPreset("openai");
    llm.set({ apiKey: "sk-test" });
    llm.applyPreset("ollama");
    llm.applyPreset("openai");
    expect(llm.apiKey).toBe("sk-test");
  });

  it("custom keeps the current fields untouched", () => {
    llm.set({ baseUrl: "https://my.endpoint/v1", model: "my-model" });
    llm.applyPreset("custom");
    expect(llm.provider).toBe("custom");
    expect(llm.baseUrl).toBe("https://my.endpoint/v1");
    expect(llm.model).toBe("my-model");
  });

  it("openai is only ready with a key", () => {
    llm.applyPreset("openai");
    llm.set({ enabled: true });
    expect(llm.ready).toBe(false);
    llm.set({ apiKey: "sk-test" });
    expect(llm.ready).toBe(true);
  });
});

describe("failure state", () => {
  it("reportError surfaces and clearError clears, without persisting", () => {
    expect(llm.lastError).toBeNull();
    llm.reportError(new ProviderError("network", "down", "start it"));
    expect(llm.lastError?.kind).toBe("network");
    expect(localStorage.getItem("pds.llm") ?? "").not.toContain("network");
    llm.clearError();
    expect(llm.lastError).toBeNull();
  });

  it("noteDrop surfaces why an answer never rendered until one does", () => {
    llm.noteDrop("invalid");
    expect(llm.lastDropReason).toBe("invalid");
    llm.clearDrop();
    expect(llm.lastDropReason).toBeNull();
  });
});

describe("usableModels", () => {
  it("narrows an OpenAI list to chat models and leaves other providers alone", () => {
    const ids = [
      "gpt-4o-mini",
      "gpt-4o-audio-preview",
      "gpt-image-1",
      "o3-mini",
      "whisper-1",
      "text-embedding-3-small",
      "dall-e-3",
      "gpt-3.5-turbo-instruct",
    ];
    expect(usableModels("openai", ids)).toEqual(["gpt-4o-mini", "o3-mini"]);
    expect(usableModels("ollama", ids)).toEqual(ids);
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
    expect(mod.llm.provider).toBe("custom"); // pre-preset entry, foreign endpoint
  });

  it("infers the preset for a pre-preset entry on a known endpoint, pinning its mode", async () => {
    localStorage.setItem(
      "pds.llm",
      JSON.stringify({ enabled: true, baseUrl: "http://localhost:11434/v1", model: "llama3", mode: "fim" }),
    );
    vi.resetModules();
    const mod = await import("./llm.svelte.js");
    expect(mod.llm.provider).toBe("ollama");
    expect(mod.llm.model).toBe("llama3");
    // The preset hides the Request-style control, so its wire shape wins over
    // the stray stored "fim".
    expect(mod.llm.mode).toBe("chat");
  });

  it("pins a preset's endpoint over a hand-edited foreign baseUrl", async () => {
    localStorage.setItem(
      "pds.llm",
      JSON.stringify({ enabled: true, provider: "openai", baseUrl: "https://evil.example/v1", apiKey: "sk-x", model: "gpt-4o-mini", mode: "chat" }),
    );
    vi.resetModules();
    const mod = await import("./llm.svelte.js");
    // The preset hides the URL field and the UI promises "sent only to
    // OpenAI" — a stored foreign endpoint must not survive.
    expect(mod.llm.provider).toBe("openai");
    expect(mod.llm.baseUrl).toBe(mod.PRESETS.openai.baseUrl);
  });

  it("survives unparseable storage", async () => {
    localStorage.setItem("pds.llm", "{nope");
    vi.resetModules();
    const mod = await import("./llm.svelte.js");
    expect(mod.llm.enabled).toBe(false);
  });
});
