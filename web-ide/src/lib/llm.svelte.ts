// AI-completion provider settings (model: ide::LlmSettingsStore). Bring-your-own
// key against any OpenAI-compatible endpoint, persisted to localStorage — the key
// never leaves the tab except on requests to the configured endpoint. Edited via
// the Settings dialog's "AI Completion" tab; the editor watches `version` to
// reconfigure the ghost-text extension when anything changes.

const STORAGE_KEY = "pds.llm";

/** The wire shape: a native fill-in-the-middle route, or the chat fallback. */
export type LlmMode = "fim" | "chat";

/** The persisted provider settings (model: ide::LlmSettings). */
export interface LlmSettings {
  enabled: boolean;
  baseUrl: string;
  apiKey: string;
  model: string;
  mode: LlmMode;
}

// Off until configured; the placeholders point at a local Ollama, the zero-key
// path (any hosted OpenAI-compatible endpoint works the same way).
export const LLM_DEFAULTS: LlmSettings = {
  enabled: false,
  baseUrl: "http://localhost:11434/v1",
  apiKey: "",
  model: "qwen2.5-coder:7b",
  mode: "chat",
};

// Load persisted settings, coercing each field so a stale or hand-edited entry
// can't poison the store (unknown mode falls back to the default).
function load(): LlmSettings {
  try {
    const obj: Record<string, unknown> = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    return {
      enabled: typeof obj.enabled === "boolean" ? obj.enabled : LLM_DEFAULTS.enabled,
      baseUrl: typeof obj.baseUrl === "string" ? obj.baseUrl : LLM_DEFAULTS.baseUrl,
      apiKey: typeof obj.apiKey === "string" ? obj.apiKey : LLM_DEFAULTS.apiKey,
      model: typeof obj.model === "string" ? obj.model : LLM_DEFAULTS.model,
      mode: obj.mode === "fim" || obj.mode === "chat" ? obj.mode : LLM_DEFAULTS.mode,
    };
  } catch {
    return { ...LLM_DEFAULTS };
  }
}

const settings: LlmSettings = $state(load());
let version: number = $state(0);

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
  /** On, and configured well enough to issue a request. */
  get ready(): boolean {
    return settings.enabled && settings.baseUrl.trim() !== "" && settings.model.trim() !== "";
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
  reset(): void {
    Object.assign(settings, LLM_DEFAULTS);
    version += 1;
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // ignore — same as persist()
    }
  },
};
