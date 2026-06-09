import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchCompletion } from "./fim-provider.js";
import type { LlmSettings } from "./llm.svelte.js";

const SETTINGS: LlmSettings = {
  enabled: true,
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

describe("failure paths", () => {
  it("rejects on a non-OK response", async () => {
    respond({}, false, 401);
    await expect(
      fetchCompletion(SETTINGS, PROMPT, new AbortController().signal),
    ).rejects.toThrow("401");
  });

  it("propagates an abort", async () => {
    fetchMock.mockImplementationOnce((_url, init: RequestInit) =>
      Promise.reject(
        Object.assign(new Error("aborted"), { name: (init.signal as AbortSignal).aborted ? "AbortError" : "Error" }),
      ),
    );
    const ctl = new AbortController();
    ctl.abort();
    await expect(fetchCompletion(SETTINGS, PROMPT, ctl.signal)).rejects.toThrow();
  });
});
