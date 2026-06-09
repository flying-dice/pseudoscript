// AI-completion provider settings (model: ide::LlmSettingsStore). Preset-first:
// picking a provider pins its endpoint and wire shape, so only the Custom preset
// exposes raw fields. Bring-your-own key against any OpenAI-compatible endpoint,
// persisted to localStorage — the key never leaves the tab except on requests to
// the configured endpoint. Edited via the Settings dialog's "AI Completion" tab;
// the editor watches `version` to reconfigure the ghost-text extension when
// anything changes. The store also carries the session's last completion failure
// (never persisted) so the status chip and the settings tab can show why ghost
// text is bailing.

import type { ProviderError } from "./fim-provider.js";

const STORAGE_KEY = "pds.llm";

/** The wire shape: a native fill-in-the-middle route, or the chat fallback. */
export type LlmMode = "fim" | "chat";

/** The provider preset: pinned endpoints, or the raw-fields escape hatch. */
export type LlmProvider = "ollama" | "openai" | "custom";

/** The persisted provider settings (model: ide::LlmSettings). */
export interface LlmSettings {
  enabled: boolean;
  provider: LlmProvider;
  baseUrl: string;
  apiKey: string;
  model: string;
  mode: LlmMode;
}

/** The fields a preset pins (model: ide::LlmSettingsStore.presetFor). The API
 * key is deliberately not part of a preset: switching providers keeps it. */
export const PRESETS: Record<Exclude<LlmProvider, "custom">, Pick<LlmSettings, "baseUrl" | "model" | "mode">> = {
  ollama: { baseUrl: "http://localhost:11434/v1", model: "qwen2.5-coder:7b", mode: "chat" },
  openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini", mode: "chat" },
};

/** The OpenAI model choices offered before the live list loads. */
export const OPENAI_MODELS = ["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini", "gpt-4.1"];

// OpenAI's /models lists every model the key can touch — embeddings, audio,
// images — most of which a /chat/completions request refuses. Keep the chat
// families and cut the modality-specific variants.
const OPENAI_CHAT = /^(gpt-|o\d)/;
const OPENAI_NON_CHAT = /audio|realtime|image|tts|transcribe|search|embed|moderation|instruct/;

/** Narrow a live /models list to what the preset's wire shape can use. */
export function usableModels(provider: LlmProvider, ids: string[]): string[] {
  if (provider !== "openai") return ids;
  return ids.filter((id) => OPENAI_CHAT.test(id) && !OPENAI_NON_CHAT.test(id));
}

// Off until configured; Ollama is the default preset — the zero-key local path.
export const LLM_DEFAULTS: LlmSettings = {
  enabled: false,
  provider: "ollama",
  apiKey: "",
  ...PRESETS.ollama,
};

// Load persisted settings, coercing each field so a stale or hand-edited entry
// can't poison the store (unknown provider/mode fall back to the defaults; an
// entry from before presets existed keeps its fields under "custom" unless they
// match the Ollama preset it was seeded with).
function load(): LlmSettings {
  try {
    const obj: Record<string, unknown> = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    const baseUrl = typeof obj.baseUrl === "string" ? obj.baseUrl : LLM_DEFAULTS.baseUrl;
    const inferred = baseUrl === PRESETS.openai.baseUrl ? "openai" : baseUrl === PRESETS.ollama.baseUrl ? "ollama" : "custom";
    const provider =
      obj.provider === "ollama" || obj.provider === "openai" || obj.provider === "custom"
        ? obj.provider
        : inferred;
    return {
      enabled: typeof obj.enabled === "boolean" ? obj.enabled : LLM_DEFAULTS.enabled,
      provider,
      baseUrl,
      apiKey: typeof obj.apiKey === "string" ? obj.apiKey : LLM_DEFAULTS.apiKey,
      model: typeof obj.model === "string" ? obj.model : LLM_DEFAULTS.model,
      // A preset pins the wire shape — the Request-style control is hidden
      // there, so a stray stored mode must not survive the migration.
      mode:
        provider !== "custom"
          ? PRESETS[provider].mode
          : obj.mode === "fim" || obj.mode === "chat"
            ? obj.mode
            : LLM_DEFAULTS.mode,
    };
  } catch {
    return { ...LLM_DEFAULTS };
  }
}

const settings: LlmSettings = $state(load());
let version: number = $state(0);
// The session's last completion failure — runtime state, never persisted.
let lastError: ProviderError | null = $state(null);
// Why the last answered suggestion never rendered (blank, or rejected by the
// parse gate) — the chip explains the silence instead of leaving the author
// wondering. Runtime state, never persisted.
let lastDrop: "empty" | "invalid" | null = $state(null);

function persist(): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // private mode / quota exceeded — settings still apply this session.
  }
}

export const llm = {
  // Bumps on every change; read it inside an effect to react to edits.
  get version() {
    return version;
  },
  get enabled() {
    return settings.enabled;
  },
  get provider() {
    return settings.provider;
  },
  get baseUrl() {
    return settings.baseUrl;
  },
  get apiKey() {
    return settings.apiKey;
  },
  get model() {
    return settings.model;
  },
  get mode() {
    return settings.mode;
  },
  /** On, and configured well enough to issue a request (OpenAI needs a key). */
  get ready(): boolean {
    return (
      settings.enabled &&
      settings.baseUrl.trim() !== "" &&
      settings.model.trim() !== "" &&
      (settings.provider !== "openai" || settings.apiKey.trim() !== "")
    );
  },
  /** The last completion failure, or null while requests succeed. */
  get lastError(): ProviderError | null {
    return lastError;
  },
  /** Why the last answer never rendered, or null once one shows. */
  get lastDropReason(): "empty" | "invalid" | null {
    return lastDrop;
  },
  /** A plain snapshot for the provider client (no reactive proxies). */
  snapshot(): LlmSettings {
    return { ...settings };
  },
  set(patch: Partial<LlmSettings>): void {
    Object.assign(settings, patch);
    version += 1;
    persist();
  },
  /** Switch preset: pin its endpoint/mode/model in one edit (model:
   * ide::LlmSettingsStore.applyPreset). "custom" keeps the fields as-is. */
  applyPreset(provider: LlmProvider): void {
    this.set(provider === "custom" ? { provider } : { provider, ...PRESETS[provider] });
  },
  /** Record a classified failure for the status chip and settings banner. */
  reportError(err: ProviderError): void {
    lastError = err;
  },
  /** Clear the failure once a request succeeds. */
  clearError(): void {
    lastError = null;
  },
  /** Record that an answer arrived but was dropped before rendering. */
  noteDrop(reason: "empty" | "invalid"): void {
    lastDrop = reason;
  },
  /** Clear the drop note once a suggestion renders. */
  clearDrop(): void {
    lastDrop = null;
  },
  reset(): void {
    Object.assign(settings, LLM_DEFAULTS);
    lastError = null;
    lastDrop = null;
    version += 1;
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // ignore — same as persist()
    }
  },
};
