import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ProviderError, fetchCompletion, listModels, testConnection, toProviderError } from "./fim-provider.js";
import type { LlmSettings } from "./llm.svelte.js";

const SETTINGS: LlmSettings = {
  enabled: true,
  provider: "custom",
  baseUrl: "https://api.example.test/v1/",
  apiKey: "sk-test",
  model: "codestral-latest",
  mode: "fim",
};
const PROMPT = { prefix: "system Banking {\n", suffix: "\n}", primer: "// grammar" };

const fetchMock = vi.fn();
beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => vi.unstubAllGlobals());

function respond(body: unknown, ok = true, status = 200): void {
  fetchMock.mockResolvedValueOnce({ ok, status, json: () => Promise.resolve(body) });
}

describe("fim mode", () => {
  it("POSTs to fim/completions with the primer prepended to the prefix", async () => {
    respond({ choices: [{ text: "ledger" }] });
    const out = await fetchCompletion(SETTINGS, PROMPT, new AbortController().signal);
    expect(out).toBe("ledger");

    const [endpoint, init] = fetchMock.mock.calls[0];
    expect(endpoint).toBe("https://api.example.test/v1/fim/completions");
    expect(init.headers.Authorization).toBe("Bearer sk-test");
    const body = JSON.parse(init.body);
    expect(body.model).toBe("codestral-latest");
    expect(body.prompt).toBe("// grammar\nsystem Banking {\n");
    expect(body.suffix).toBe("\n}");
  });

  it("reads Mistral's chat-shaped FIM answer", async () => {
    respond({ choices: [{ message: { content: "ledger" } }] });
    await expect(fetchCompletion(SETTINGS, PROMPT, new AbortController().signal)).resolves.toBe(
      "ledger",
    );
  });
});

describe("chat mode", () => {
  const chat: LlmSettings = { ...SETTINGS, mode: "chat", apiKey: "" };

  it("POSTs FIM-shaped messages and omits Authorization without a key", async () => {
    respond({ choices: [{ message: { content: "ledger" } }] });
    const out = await fetchCompletion(chat, PROMPT, new AbortController().signal);
    expect(out).toBe("ledger");

    const [endpoint, init] = fetchMock.mock.calls[0];
    expect(endpoint).toBe("https://api.example.test/v1/chat/completions");
    expect(init.headers.Authorization).toBeUndefined();
    const body = JSON.parse(init.body);
    expect(body.messages[0].role).toBe("system");
    expect(body.messages[0].content).toContain("// grammar");
    expect(body.messages[1].content).toBe("system Banking {\n<CURSOR>\n}");
  });

  it("strips markdown fences from a chatty answer", async () => {
    respond({ choices: [{ message: { content: "```pds\nledger\n```" } }] });
    await expect(fetchCompletion(chat, PROMPT, new AbortController().signal)).resolves.toBe(
      "ledger",
    );
  });

  it("resolves empty when the provider returns no choices", async () => {
    respond({});
    await expect(fetchCompletion(chat, PROMPT, new AbortController().signal)).resolves.toBe("");
  });
});

describe("failure classification", () => {
  async function failure(status: number, settings: LlmSettings = SETTINGS): Promise<ProviderError> {
    respond({}, false, status);
    try {
      await fetchCompletion(settings, PROMPT, new AbortController().signal);
    } catch (e) {
      return e as ProviderError;
    }
    throw new Error("expected a rejection");
  }

  it("classifies 401/403 as auth with a key hint", async () => {
    const err = await failure(401);
    expect(err).toBeInstanceOf(ProviderError);
    expect(err.kind).toBe("auth");
    expect(err.hint).toContain("API key");
  });

  it("classifies 404 as notFound, with the ollama pull hint on that preset", async () => {
    const err = await failure(404, { ...SETTINGS, provider: "ollama", model: "qwen2.5-coder:7b" });
    expect(err.kind).toBe("notFound");
    expect(err.hint).toContain("ollama pull qwen2.5-coder:7b");
  });

  it("classifies other non-OK statuses as http", async () => {
    const err = await failure(500);
    expect(err.kind).toBe("http");
    expect(err.message).toContain("500");
  });

  it("classifies a fetch TypeError as network, with the CORS hint for ollama", async () => {
    fetchMock.mockRejectedValueOnce(new TypeError("Failed to fetch"));
    const ollama: LlmSettings = { ...SETTINGS, provider: "ollama" };
    try {
      await fetchCompletion(ollama, PROMPT, new AbortController().signal);
      throw new Error("expected a rejection");
    } catch (e) {
      const err = e as ProviderError;
      expect(err.kind).toBe("network");
      expect(err.hint).toContain("OLLAMA_ORIGINS");
    }
  });

  it("rethrows the caller's own abort unclassified", async () => {
    fetchMock.mockImplementationOnce((_url, init: RequestInit) =>
      Promise.reject(
        Object.assign(new Error("aborted"), { name: (init.signal as AbortSignal).aborted ? "AbortError" : "Error" }),
      ),
    );
    const ctl = new AbortController();
    ctl.abort();
    await expect(fetchCompletion(SETTINGS, PROMPT, ctl.signal)).rejects.toThrow("aborted");
  });

  it("classifies a 200 with a non-JSON body instead of leaking a SyntaxError", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: () => Promise.reject(new SyntaxError("Unexpected token <")),
    });
    try {
      await fetchCompletion(SETTINGS, PROMPT, new AbortController().signal);
      throw new Error("expected a rejection");
    } catch (e) {
      expect(e).toBeInstanceOf(ProviderError);
      expect((e as ProviderError).kind).toBe("http");
    }
  });

  it("toProviderError wraps a stray error as http and passes a ProviderError through", () => {
    const wrapped = toProviderError(new Error("boom"));
    expect(wrapped.kind).toBe("http");
    expect(wrapped.message).toBe("boom");
    const original = new ProviderError("auth", "no", "fix");
    expect(toProviderError(original)).toBe(original);
  });
});

describe("listModels and testConnection", () => {
  it("GETs /models and returns the ids", async () => {
    respond({ data: [{ id: "qwen2.5-coder:7b" }, { id: "llama3" }, {}] });
    const models = await listModels(SETTINGS, new AbortController().signal);
    expect(models).toEqual(["qwen2.5-coder:7b", "llama3"]);
    const [endpoint, init] = fetchMock.mock.calls[0];
    expect(endpoint).toBe("https://api.example.test/v1/models");
    expect(init.method).toBe("GET");
    expect(init.body).toBeUndefined();
  });

  it("testConnection lists models then probes a completion", async () => {
    respond({ data: [{ id: "m1" }] });
    respond({ choices: [{ text: "ok" }] });
    const models = await testConnection(SETTINGS, new AbortController().signal);
    expect(models).toEqual(["m1"]);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][0]).toBe("https://api.example.test/v1/fim/completions");
  });

  it("testConnection surfaces the first classified failure", async () => {
    respond({}, false, 403);
    await expect(
      testConnection(SETTINGS, new AbortController().signal),
    ).rejects.toMatchObject({ kind: "auth" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
