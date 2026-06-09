// Provider-agnostic completion client (model: ide::FimClient): one `fetchCompletion`
// contract over OpenAI-compatible HTTP, so a single client covers Codestral,
// DeepSeek, OpenRouter, Together, Ollama, and vLLM. The wire shape is picked by
// the configured mode: a native fill-in-the-middle route, or a FIM-shaped chat
// request for providers without one. Callers own cancellation via `signal`;
// every request also carries its own timeout.

import type { FimPrompt } from "./fim-context.js";
import type { LlmSettings } from "./llm.svelte.js";

// Small completions, low temperature: ghost text proposes the next few tokens /
// line, not a whole module.
const MAX_TOKENS = 128;
const TEMPERATURE = 0.2;
const TIMEOUT_MS = 10_000;

// The subset of an OpenAI-compatible completion response this client reads.
// Native FIM routes answer with `text`; chat (and Mistral's FIM) with
// `message.content`.
interface CompletionResponse {
  choices?: { text?: string; message?: { content?: string } }[];
}

function url(baseUrl: string, route: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/${route}`;
}

function headers(apiKey: string): Record<string, string> {
  const base: Record<string, string> = { "Content-Type": "application/json" };
  if (apiKey.trim()) base.Authorization = `Bearer ${apiKey.trim()}`;
  return base;
}

// A chat model loves to wrap code in markdown fences and echo context; keep only
// the insertion. Inner backticks are left alone — `.pds` has none anyway.
function stripFences(text: string): string {
  const fenced = text.match(/^\s*```[^\n]*\n([\s\S]*?)\n?```\s*$/);
  return (fenced ? fenced[1] : text).replace(/\r/g, "");
}

async function post(
  endpoint: string,
  apiKey: string,
  body: unknown,
  signal: AbortSignal,
): Promise<CompletionResponse> {
  const timeout = AbortSignal.timeout(TIMEOUT_MS);
  const res = await fetch(endpoint, {
    method: "POST",
    headers: headers(apiKey),
    body: JSON.stringify(body),
    signal: AbortSignal.any([signal, timeout]),
  });
  if (!res.ok) throw new Error(`completion request failed: ${res.status}`);
  return (await res.json()) as CompletionResponse;
}

// Native FIM (`/fim/completions`, Codestral/DeepSeek shape). The primer is
// prepended to the prefix: it is written as `.pds` comments, so the model reads
// it as in-file context.
async function fimComplete(
  settings: LlmSettings,
  prompt: FimPrompt,
  signal: AbortSignal,
): Promise<string> {
  const data = await post(
    url(settings.baseUrl, "fim/completions"),
    settings.apiKey,
    {
      model: settings.model,
      prompt: `${prompt.primer}\n${prompt.prefix}`,
      suffix: prompt.suffix,
      max_tokens: MAX_TOKENS,
      temperature: TEMPERATURE,
    },
    signal,
  );
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
  const data = await post(
    url(settings.baseUrl, "chat/completions"),
    settings.apiKey,
    {
      model: settings.model,
      messages: [
        { role: "system", content: system },
        { role: "user", content: `${prompt.prefix}<CURSOR>${prompt.suffix}` },
      ],
      max_tokens: MAX_TOKENS,
      temperature: TEMPERATURE,
    },
    signal,
  );
  return stripFences(data.choices?.[0]?.message?.content ?? "");
}

/**
 * Ask the configured provider to fill the middle. Resolves to the raw insertion
 * ("" when the provider returns nothing); rejects on abort, timeout, or a non-OK
 * response — the ghost-text host treats any rejection as "no suggestion".
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
