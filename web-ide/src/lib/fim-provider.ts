// Provider-agnostic completion client (model: ide::FimClient): one `fetchCompletion`
// contract over OpenAI-compatible HTTP, so a single client covers OpenAI, Codestral,
// DeepSeek, OpenRouter, Together, Ollama, and vLLM. The wire shape is picked by
// the configured mode: a native fill-in-the-middle route, or a FIM-shaped chat
// request for providers without one. Callers own cancellation via `signal`;
// every request also carries its own timeout. Every failure is a classified
// `ProviderError` (model: ide::ProviderError), so callers can tell the author
// why a request bailed and what fixes it.

import type { FimPrompt } from "./fim-context.js";
import type { LlmSettings } from "./llm.svelte.js";

// Small completions, low temperature: ghost text proposes the next few tokens /
// line, not a whole module.
const MAX_TOKENS = 128;
const TEMPERATURE = 0.2;
const TIMEOUT_MS = 10_000;

/** Why a provider request failed, coarsely — each kind has one fix. */
export type ProviderErrorKind = "network" | "auth" | "notFound" | "timeout" | "http";

/** A classified provider failure: what happened, and what fixes it. */
export class ProviderError extends Error {
  constructor(
    readonly kind: ProviderErrorKind,
    message: string,
    readonly hint: string,
  ) {
    super(message);
    this.name = "ProviderError";
  }
}

// A fetch TypeError means the browser never got a response: the endpoint is
// down, unreachable, or the response was CORS-blocked. The hint names the
// likeliest fix per provider — for a local Ollama that is almost always the
// daemon not running or OLLAMA_ORIGINS not allowing this site.
function networkHint(settings: LlmSettings): string {
  if (settings.provider === "ollama") {
    return (
      "Check Ollama is running (`ollama serve`). If it is, the browser is likely CORS-blocked — " +
      'restart it with this site allowed, e.g. `OLLAMA_ORIGINS="*" ollama serve` ' +
      '(macOS app: `launchctl setenv OLLAMA_ORIGINS "*"` then restart Ollama).'
    );
  }
  return "Check the endpoint URL is reachable from this browser and that the provider allows cross-origin (CORS) requests.";
}

function classify(err: unknown, settings: LlmSettings): ProviderError {
  if (err instanceof ProviderError) return err;
  if (err instanceof DOMException && err.name === "TimeoutError") {
    return new ProviderError(
      "timeout",
      `No response within ${TIMEOUT_MS / 1000}s from ${settings.baseUrl}`,
      "The endpoint is up but slow — a smaller model or a closer endpoint responds in time.",
    );
  }
  if (err instanceof TypeError) {
    return new ProviderError(
      "network",
      `Could not reach ${settings.baseUrl}`,
      networkHint(settings),
    );
  }
  return new ProviderError(
    "http",
    err instanceof Error ? err.message : String(err),
    "Check the endpoint URL and model name in Settings → AI Completion.",
  );
}

function statusError(status: number, settings: LlmSettings): ProviderError {
  if (status === 401 || status === 403) {
    return new ProviderError(
      "auth",
      `The provider rejected the API key (HTTP ${status})`,
      settings.provider === "openai"
        ? "Paste a valid OpenAI API key in Settings → AI Completion."
        : "Check the API key in Settings → AI Completion (leave it empty for a local model).",
    );
  }
  if (status === 404) {
    return new ProviderError(
      "notFound",
      "The provider answered 404 — unknown route or model",
      settings.provider === "ollama"
        ? `Pull the model first: \`ollama pull ${settings.model}\`.`
        : "Check the model name, and that the endpoint speaks the OpenAI API.",
    );
  }
  return new ProviderError(
    "http",
    `The provider answered HTTP ${status}`,
    "Check the provider's status and the configuration in Settings → AI Completion.",
  );
}

/** Coerce any rejection into a `ProviderError` for the status surfaces. */
export function toProviderError(err: unknown): ProviderError {
  if (err instanceof ProviderError) return err;
  return new ProviderError(
    "http",
    err instanceof Error ? err.message : String(err),
    "Check the configuration in Settings → AI Completion.",
  );
}

// The subset of an OpenAI-compatible completion response this client reads.
// Native FIM routes answer with `text`; chat (and Mistral's FIM) with
// `message.content`.
interface CompletionResponse {
  choices?: { text?: string; message?: { content?: string } }[];
}

function url(baseUrl: string, route: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/${route}`;
}

// Content-Type only when a body crosses — a bare GET stays preflight-free.
function headers(apiKey: string, hasBody: boolean): Record<string, string> {
  const base: Record<string, string> = hasBody ? { "Content-Type": "application/json" } : {};
  if (apiKey.trim()) base.Authorization = `Bearer ${apiKey.trim()}`;
  return base;
}

// A chat model loves to wrap code in markdown fences and echo context; keep only
// the insertion. Inner backticks are left alone — `.pds` has none anyway.
function stripFences(text: string): string {
  const fenced = text.match(/^\s*```[^\n]*\n([\s\S]*?)\n?```\s*$/);
  return (fenced ? fenced[1] : text).replace(/\r/g, "");
}

async function request(
  settings: LlmSettings,
  route: string,
  init: { method: "GET" | "POST"; body?: unknown },
  signal: AbortSignal,
): Promise<unknown> {
  const timeout = AbortSignal.timeout(TIMEOUT_MS);
  try {
    const res = await fetch(url(settings.baseUrl, route), {
      method: init.method,
      headers: headers(settings.apiKey, init.body !== undefined),
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      signal: AbortSignal.any([signal, timeout]),
    });
    if (!res.ok) throw statusError(res.status, settings);
    // Inside the try: a 200 carrying a non-JSON body (a proxy's HTML error
    // page) classifies instead of escaping as a raw SyntaxError.
    return await res.json();
  } catch (e) {
    // The caller's own abort is not a failure to report.
    if (signal.aborted) throw e;
    throw classify(e, settings);
  }
}

// Native FIM (`/fim/completions`, Codestral/DeepSeek shape). The primer is
// prepended to the prefix: it is written as `.pds` comments, so the model reads
// it as in-file context.
async function fimComplete(
  settings: LlmSettings,
  prompt: FimPrompt,
  signal: AbortSignal,
): Promise<string> {
  const data = (await request(
    settings,
    "fim/completions",
    {
      method: "POST",
      body: {
        model: settings.model,
        prompt: `${prompt.primer}\n${prompt.prefix}`,
        suffix: prompt.suffix,
        max_tokens: MAX_TOKENS,
        temperature: TEMPERATURE,
      },
    },
    signal,
  )) as CompletionResponse;
  const choice = data.choices?.[0];
  return choice?.message?.content ?? choice?.text ?? "";
}

// Chat fallback (`/chat/completions`): the primer becomes the system prompt and
// the prefix/suffix cross as a marked-up document the model fills at <CURSOR>.
async function chatComplete(
  settings: LlmSettings,
  prompt: FimPrompt,
  signal: AbortSignal,
): Promise<string> {
  const system =
    `You are a code-completion engine for PseudoScript (.pds).\n${prompt.primer}\n` +
    "Insert code at <CURSOR>. Reply with ONLY the text to insert — " +
    "no explanation, no markdown fences, never repeat the surrounding code.";
  const data = (await request(
    settings,
    "chat/completions",
    {
      method: "POST",
      body: {
        model: settings.model,
        messages: [
          { role: "system", content: system },
          { role: "user", content: `${prompt.prefix}<CURSOR>${prompt.suffix}` },
        ],
        max_tokens: MAX_TOKENS,
        temperature: TEMPERATURE,
      },
    },
    signal,
  )) as CompletionResponse;
  return stripFences(data.choices?.[0]?.message?.content ?? "");
}

/**
 * Ask the configured provider to fill the middle. Resolves to the raw insertion
 * ("" when the provider returns nothing); rejects with a `ProviderError` on
 * timeout or a non-OK response (a caller-initiated abort rethrows as-is).
 */
export function fetchCompletion(
  settings: LlmSettings,
  prompt: FimPrompt,
  signal: AbortSignal,
): Promise<string> {
  return settings.mode === "fim"
    ? fimComplete(settings, prompt, signal)
    : chatComplete(settings, prompt, signal);
}

/**
 * The provider's live model ids (`GET /models`, OpenAI shape — a local Ollama
 * answers with the models the host has pulled). Feeds the settings tab's model
 * dropdown; rejects with a `ProviderError`.
 */
export async function listModels(settings: LlmSettings, signal: AbortSignal): Promise<string[]> {
  const data = (await request(settings, "models", { method: "GET" }, signal)) as {
    data?: { id?: string }[];
  };
  return (data.data ?? []).map((m) => m.id ?? "").filter((id) => id !== "");
}

// What the connection test asks the model to complete — tiny on purpose.
const PROBE: FimPrompt = { prefix: "// PseudoScript\nsystem ", suffix: "", primer: "" };

/**
 * Round-trip probe for the settings tab's "Test connection": list the models,
 * then ask for a one-token completion. Resolves to the model ids; rejects with
 * the first classified `ProviderError`.
 */
export async function testConnection(
  settings: LlmSettings,
  signal: AbortSignal,
): Promise<string[]> {
  const models = await listModels(settings, signal);
  await fetchCompletion(settings, PROBE, signal);
  return models;
}
