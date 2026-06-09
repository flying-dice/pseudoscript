<script lang="ts">
  import type { Command } from "$lib/keybindings.svelte.js";
  import { COMMANDS, PROFILES, chordFromEvent, formatChord, keybindings } from "$lib/keybindings.svelte.js";
  import type { LlmProvider } from "$lib/llm.svelte.js";
  import { OPENAI_MODELS, llm, usableModels } from "$lib/llm.svelte.js";
  import { listModels, testConnection, toProviderError } from "$lib/fim-provider.js";

  type Props = {
    onclose: () => void;
    /** The page to open on — lets a caller land on the tab it advertised. */
    initialTab?: "keyboard" | "ai";
  };

  let { onclose, initialTab = "keyboard" }: Props = $props();

  // The open settings page. Keyboard by default — it predates the AI tab and
  // the shell's "keyboard shortcuts" actions land here. The prop is a seed read
  // once at mount (the dialog is recreated per open), not a live binding.
  // svelte-ignore state_referenced_locally
  let tab = $state<"keyboard" | "ai">(initialTab);

  // A command row enriched with its effective chord and customised flag.
  type Row = Command & { chord: string; custom: boolean };
  // A display group: a named bucket of command rows.
  type Group = { name: string; items: Row[] };

  // The command id currently capturing a chord, or null. While set, a global
  // capture-phase listener swallows every keydown and rebinds on the first
  // non-modifier key (Escape cancels).
  let recording = $state<string | null>(null);
  // A transient inline message under the row being recorded (e.g. a conflict).
  let notice = $state<string | null>(null);

  // Rows grouped for display, recomputed when a binding changes (version).
  const groups: Group[] = $derived.by(() => {
    keybindings.version; // track rebinds so chips refresh
    const by = new Map<string, Row[]>();
    for (const c of COMMANDS) {
      if (!by.has(c.group)) by.set(c.group, []);
      by.get(c.group)!.push({
        ...c,
        chord: keybindings.keyFor(c.id),
        custom: keybindings.isCustom(c.id),
      });
    }
    return [...by.entries()].map(([name, items]) => ({ name, items }));
  });

  // --- AI Completion tab state ---------------------------------------------

  const PROVIDERS: { id: LlmProvider; label: string; blurb: string }[] = [
    { id: "ollama", label: "Ollama (local)", blurb: "Runs on this machine — free, no key, code never leaves it." },
    { id: "openai", label: "OpenAI", blurb: "Hosted — bring your own API key." },
    { id: "custom", label: "Custom", blurb: "Any OpenAI-compatible endpoint." },
  ];

  // The provider's live model list (Ollama: what the host has pulled; OpenAI:
  // what the key can use). null = not loaded (fall back to a text input /
  // static list); refreshed whenever the provider or its credentials change.
  let liveModels = $state<string[] | null>(null);
  let modelsBusy = $state(false);
  // Whether the last list fetch failed outright — a reachable provider with an
  // empty list (no models pulled yet) is a different problem than no answer,
  // and the hint under the model field says which one the author has.
  let modelsUnreachable = $state(false);
  // Last provider a list was fetched for — plain (non-reactive) on purpose, so
  // a failed fetch doesn't re-trigger the loading effect in a loop.
  let modelsFetchedFor: string | null = null;

  // The connection test's outcome, shown inline under the button.
  let testBusy = $state(false);
  let testResult = $state<{ ok: boolean; message: string; hint?: string } | null>(null);

  // Adopt a fetch outcome only when it's still for the active provider (a slow
  // response must not land in another preset's dropdown), and only the usable
  // slice of a list — an empty slice falls back to the text input / static list.
  function adoptModels(forProvider: LlmProvider, outcome: string[] | "unreachable"): void {
    if (llm.provider !== forProvider) return;
    modelsUnreachable = outcome === "unreachable";
    const usable = outcome === "unreachable" ? null : usableModels(forProvider, outcome);
    liveModels = usable?.length ? usable : null;
  }

  async function refreshModels(): Promise<void> {
    const forProvider = llm.provider;
    modelsBusy = true;
    modelsFetchedFor = forProvider;
    try {
      adoptModels(forProvider, await listModels(llm.snapshot(), AbortSignal.timeout(5000)));
    } catch {
      adoptModels(forProvider, "unreachable");
    } finally {
      modelsBusy = false;
    }
  }

  function pickProvider(id: LlmProvider): void {
    llm.applyPreset(id);
    testResult = null;
    liveModels = null;
    modelsUnreachable = false;
    modelsFetchedFor = null;
    if (id !== "custom") void refreshModels();
  }

  async function runTest(): Promise<void> {
    const forProvider = llm.provider;
    testBusy = true;
    testResult = null;
    try {
      const models = await testConnection(llm.snapshot(), new AbortController().signal);
      // Count what the dropdown will actually offer, not everything the key
      // can touch (OpenAI's raw list includes audio/embedding models).
      const usable = usableModels(forProvider, models);
      testResult = { ok: true, message: `Connected — ${usable.length} model(s) available.` };
      llm.clearError();
      adoptModels(forProvider, models);
    } catch (e) {
      const err = toProviderError(e);
      testResult = { ok: false, message: err.message, hint: err.hint };
    } finally {
      testBusy = false;
    }
  }

  // The dropdown's choices: the live list when it loaded, else OpenAI's static
  // fallback. The configured model is always offered so the selection sticks.
  const modelChoices = $derived.by(() => {
    const base = liveModels ?? (llm.provider === "openai" ? OPENAI_MODELS : null);
    if (!base) return null;
    return base.includes(llm.model) || llm.model.trim() === "" ? base : [llm.model, ...base];
  });

  // Load the model list when the tab opens on a preset provider — once per
  // provider pick; "Test connection" is the explicit retry.
  $effect(() => {
    if (tab === "ai" && llm.provider !== "custom" && modelsFetchedFor !== llm.provider) {
      void refreshModels();
    }
  });

  // --------------------------------------------------------------------------

  function startRecording(id: string): void {
    recording = id;
    notice = null;
  }
  function stopRecording(): void {
    recording = null;
    notice = null;
  }

  // Capture a chord for the recording command. Runs in the capture phase so the
  // app's own shortcuts (and the browser's) never fire while recording.
  function onKeydown(e: KeyboardEvent): void {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") return stopRecording();
    const chord = chordFromEvent(e);
    if (!chord) return; // lone modifier — keep waiting
    const clash = keybindings.conflict(chord, recording);
    if (clash) {
      const label = COMMANDS.find((c) => c.id === clash)?.label ?? clash;
      notice = `${formatChord(chord)} is already used by “${label}”.`;
      return;
    }
    keybindings.setKey(recording, chord);
    stopRecording();
  }
</script>

<svelte:window onkeydowncapture={onKeydown} />

<div class="scrim" role="presentation" onclick={onclose}></div>
<div class="panel" role="dialog" aria-modal="true" aria-label="Settings" data-testid="settings-dialog">
  <header>
    <h2>Settings</h2>
    <button class="x" aria-label="Close" onclick={onclose}>✕</button>
  </header>

  <div class="tabs" role="tablist" aria-label="Settings sections">
    <button
      role="tab"
      aria-selected={tab === "keyboard"}
      class:active={tab === "keyboard"}
      data-testid="settings-tab-keyboard"
      onclick={() => (tab = "keyboard")}>Keyboard</button
    >
    <button
      role="tab"
      aria-selected={tab === "ai"}
      class:active={tab === "ai"}
      data-testid="settings-tab-ai"
      onclick={() => (tab = "ai")}>AI Completion</button
    >
  </div>

  {#if tab === "ai"}
    <div class="body ai" data-testid="llm-panel">
      <label class="toggle">
        <input
          type="checkbox"
          data-testid="llm-enabled"
          checked={llm.enabled}
          onchange={(e) => llm.set({ enabled: e.currentTarget.checked })}
        />
        <span>Enable inline AI completion</span>
      </label>
      <p class="blurb">
        Ghost-text suggestions while you type. Pick where they come from — grammar autocomplete
        stays local either way.
      </p>

      {#if llm.lastError}
        <div class="ai-error" data-testid="llm-last-error">
          <p class="ai-error-msg">{llm.lastError.message}</p>
          <p class="ai-error-hint">{llm.lastError.hint}</p>
        </div>
      {/if}

      <div class="providers" role="radiogroup" aria-label="Completion provider">
        {#each PROVIDERS as p (p.id)}
          <button
            class="provider"
            class:active={llm.provider === p.id}
            role="radio"
            aria-checked={llm.provider === p.id}
            data-testid="llm-provider-{p.id}"
            onclick={() => pickProvider(p.id)}
          >
            <span class="p-label">{p.label}</span>
            <span class="p-blurb">{p.blurb}</span>
          </button>
        {/each}
      </div>

      {#snippet apiKeyField(placeholder: string, hint: string)}
        <div class="field">
          <label for="llm-apikey">API key</label>
          <input
            id="llm-apikey"
            type="password"
            autocomplete="off"
            data-testid="llm-apikey"
            {placeholder}
            value={llm.apiKey}
            oninput={(e) => llm.set({ apiKey: e.currentTarget.value })}
            onchange={() => {
              // A committed key unlocks the preset's live model list.
              if (llm.provider !== "custom") void refreshModels();
            }}
          />
          <p class="hint-line">{hint}</p>
        </div>
      {/snippet}

      {#if llm.provider === "openai"}
        {@render apiKeyField("sk-…", "Stored in this browser only; sent only to OpenAI.")}
      {/if}

      {#if llm.provider === "custom"}
        <div class="field">
          <label for="llm-baseurl">Endpoint base URL</label>
          <input
            id="llm-baseurl"
            type="url"
            data-testid="llm-baseurl"
            placeholder="http://localhost:11434/v1"
            value={llm.baseUrl}
            oninput={(e) => llm.set({ baseUrl: e.currentTarget.value })}
          />
        </div>
        {@render apiKeyField(
          "empty for a local model",
          "Stored in this browser only; sent only to the endpoint above.",
        )}
        <div class="field">
          <label for="llm-model">Model</label>
          <input
            id="llm-model"
            type="text"
            data-testid="llm-model"
            placeholder="qwen2.5-coder:7b"
            value={llm.model}
            oninput={(e) => llm.set({ model: e.currentTarget.value })}
          />
        </div>
        <div class="field">
          <label for="llm-mode">Request style</label>
          <select
            id="llm-mode"
            data-testid="llm-mode"
            value={llm.mode}
            onchange={(e) => llm.set({ mode: e.currentTarget.value === "fim" ? "fim" : "chat" })}
          >
            <option value="chat">Chat (works everywhere)</option>
            <option value="fim">Native fill-in-the-middle</option>
          </select>
          <p class="hint-line">
            Chat suits Ollama / OpenRouter / vLLM; native FIM suits Codestral and DeepSeek.
          </p>
        </div>
      {:else}
        <div class="field">
          <label for="llm-model">Model</label>
          {#if modelChoices}
            <select
              id="llm-model"
              data-testid="llm-model"
              value={llm.model}
              onchange={(e) => llm.set({ model: e.currentTarget.value })}
            >
              {#each modelChoices as m (m)}
                <option value={m}>{m}</option>
              {/each}
            </select>
          {:else}
            <input
              id="llm-model"
              type="text"
              data-testid="llm-model"
              placeholder={llm.provider === "ollama" ? "qwen2.5-coder:7b" : "gpt-4o-mini"}
              value={llm.model}
              oninput={(e) => llm.set({ model: e.currentTarget.value })}
            />
          {/if}
          <p class="hint-line">
            {#if modelsBusy}
              Loading the model list…
            {:else if llm.provider === "ollama"}
              {liveModels
                ? "Models installed on your local Ollama."
                : modelsUnreachable
                  ? "Couldn't reach your local Ollama for its model list — type a model you've pulled (e.g. `ollama pull qwen2.5-coder:7b`)."
                  : "Ollama is reachable but has no models yet — run `ollama pull qwen2.5-coder:7b`, then Test connection."}
            {:else}
              {liveModels ? "Models your API key can use." : "Common choices — the live list loads once the key works."}
            {/if}
          </p>
        </div>
      {/if}

      <div class="test-row">
        <button class="test" data-testid="llm-test" disabled={testBusy} onclick={() => void runTest()}>
          {testBusy ? "Testing…" : "Test connection"}
        </button>
      </div>
      {#if testResult}
        <div class="test-result" class:ok={testResult.ok} class:fail={!testResult.ok} data-testid="llm-test-result">
          <p class="test-msg">{testResult.ok ? "✓" : "✕"} {testResult.message}</p>
          {#if testResult.hint}<p class="test-hint">{testResult.hint}</p>{/if}
        </div>
      {/if}
    </div>
  {:else}
  <div class="profile-bar">
    <label for="keymap-profile">Keymap</label>
    <select
      id="keymap-profile"
      value={keybindings.profile}
      onchange={(e) => keybindings.setProfile(e.currentTarget.value)}
    >
      {#each PROFILES as p (p.id)}
        <option value={p.id}>{p.label}</option>
      {/each}
    </select>
    <span class="profile-hint">Preset bindings; your edits layer on top.</span>
  </div>

  <div class="body">
    {#each groups as group (group.name)}
      <section>
        <h3>{group.name}</h3>
        <ul>
          {#each group.items as cmd (cmd.id)}
            <li>
              <span class="label">{cmd.label}</span>
              <div class="binding">
                {#if recording === cmd.id}
                  <button class="chord recording" data-testid="keybind-{cmd.id}" onclick={stopRecording}>Press keys… (Esc)</button>
                {:else}
                  <button class="chord" class:custom={cmd.custom} data-testid="keybind-{cmd.id}" onclick={() => startRecording(cmd.id)}>
                    {formatChord(cmd.chord)}
                  </button>
                {/if}
                <button
                  class="reset"
                  title="Reset to default"
                  data-testid="keybind-reset-{cmd.id}"
                  disabled={!cmd.custom}
                  onclick={() => keybindings.reset(cmd.id)}>↺</button
                >
              </div>
              {#if recording === cmd.id && notice}
                <p class="notice" data-testid="keybind-conflict">{notice}</p>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  <footer>
    <span class="hint">Click a shortcut, then press the new key combination.</span>
    <button class="reset-all" data-testid="settings-reset-all" onclick={() => keybindings.resetAll()}>Reset all</button>
  </footer>
  {/if}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(2px);
  }
  .panel {
    position: fixed;
    z-index: 51;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(34rem, 92vw);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.7rem 0.9rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface-2);
  }
  h2 {
    margin: 0;
    font-family: var(--font-display, var(--font-sans));
    font-size: 0.95rem;
    color: var(--ink);
  }
  .x {
    background: transparent;
    border: none;
    color: var(--ink-faint);
    font-size: 0.85rem;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: var(--radius-sm);
  }
  .x:hover {
    color: var(--ink);
    background: var(--surface-3);
  }
  .tabs {
    display: flex;
    gap: 0.2rem;
    padding: 0.4rem 0.9rem 0;
    border-bottom: 1px solid var(--line);
    background: var(--surface-2);
  }
  .tabs button {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 0.7rem;
    letter-spacing: 0.06em;
    padding: 0.35rem 0.6rem;
    cursor: pointer;
  }
  .tabs button:hover {
    color: var(--ink);
  }
  .tabs button.active {
    color: var(--accent-hi);
    border-bottom-color: var(--accent);
  }
  .body.ai {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding-top: 0.9rem;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    color: var(--ink);
    cursor: pointer;
  }
  .toggle input {
    accent-color: var(--accent);
  }
  .blurb {
    margin: 0;
    font-size: 0.72rem;
    color: var(--ink-soft);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .field label {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .field input,
  .field select {
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.76rem;
    padding: 0.35rem 0.5rem;
  }
  .field input:focus,
  .field select:focus {
    outline: none;
    border-color: var(--accent);
  }
  .hint-line {
    margin: 0;
    font-size: 0.66rem;
    color: var(--ink-faint);
  }
  .providers {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.5rem;
  }
  .provider {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    text-align: left;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.55rem;
    cursor: pointer;
  }
  .provider:hover {
    border-color: var(--accent);
  }
  .provider.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .p-label {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    letter-spacing: 0.04em;
    color: var(--ink);
  }
  .provider.active .p-label {
    color: var(--accent-hi);
  }
  .p-blurb {
    font-size: 0.62rem;
    line-height: 1.35;
    color: var(--ink-faint);
  }
  .ai-error {
    border: 1px solid color-mix(in srgb, var(--err) 45%, var(--line-strong));
    border-left: 3px solid var(--err);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--err) 7%, var(--surface-2));
    padding: 0.5rem 0.6rem;
  }
  .ai-error-msg {
    margin: 0;
    font-size: 0.72rem;
    color: var(--ink);
  }
  .ai-error-hint,
  .test-hint {
    margin: 0.25rem 0 0;
    font-size: 0.66rem;
    line-height: 1.4;
    color: var(--ink-soft);
    word-break: break-word;
  }
  .test-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .test {
    background: transparent;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.04em;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
  }
  .test:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--ink);
  }
  .test:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .test-result {
    border-radius: var(--radius-sm);
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--line-strong);
  }
  .test-result.ok {
    border-color: color-mix(in srgb, var(--ok) 45%, var(--line-strong));
    background: color-mix(in srgb, var(--ok) 7%, var(--surface-2));
  }
  .test-result.fail {
    border-color: color-mix(in srgb, var(--err) 45%, var(--line-strong));
    background: color-mix(in srgb, var(--err) 7%, var(--surface-2));
  }
  .test-msg {
    margin: 0;
    font-size: 0.72rem;
    color: var(--ink);
  }
  .profile-bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--line);
  }
  .profile-bar label {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .profile-bar select {
    appearance: none;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.74rem;
    padding: 0.2rem 1.6rem 0.2rem 0.5rem;
    cursor: pointer;
    background-image: linear-gradient(45deg, transparent 50%, var(--ink-faint) 50%),
      linear-gradient(135deg, var(--ink-faint) 50%, transparent 50%);
    background-position:
      right 0.7rem center,
      right 0.45rem center;
    background-size:
      0.3rem 0.3rem,
      0.3rem 0.3rem;
    background-repeat: no-repeat;
  }
  .profile-bar select:hover {
    border-color: var(--accent);
  }
  .profile-hint {
    margin-left: auto;
    font-size: 0.66rem;
    color: var(--ink-faint);
  }
  .body {
    overflow: auto;
    padding: 0.5rem 0.9rem 0.9rem;
  }
  section {
    margin-top: 0.7rem;
  }
  h3 {
    margin: 0 0 0.3rem;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 0.6rem;
    padding: 0.32rem 0;
    border-bottom: 1px solid var(--line);
  }
  li:last-child {
    border-bottom: none;
  }
  .label {
    font-size: 0.82rem;
    color: var(--ink-soft);
  }
  .binding {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .chord {
    min-width: 5.5rem;
    padding: 0.22rem 0.5rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.74rem;
    letter-spacing: 0.05em;
    cursor: pointer;
    text-align: center;
  }
  .chord:hover {
    border-color: var(--accent);
  }
  .chord.custom {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--line-strong));
    color: var(--accent-hi);
  }
  .chord.recording {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
    animation: pulse 1.1s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.6;
    }
  }
  .reset {
    background: transparent;
    border: none;
    color: var(--ink-faint);
    font-size: 0.85rem;
    cursor: pointer;
    padding: 0.1rem 0.25rem;
    border-radius: var(--radius-sm);
  }
  .reset:hover:not(:disabled) {
    color: var(--accent);
  }
  .reset:disabled {
    opacity: 0.25;
    cursor: default;
  }
  .notice {
    grid-column: 1 / -1;
    margin: 0.2rem 0 0;
    font-size: 0.68rem;
    color: var(--warn);
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    padding: 0.6rem 0.9rem;
    border-top: 1px solid var(--line);
    background: var(--surface-2);
  }
  .hint {
    margin: 0;
    font-size: 0.68rem;
    color: var(--ink-faint);
  }
  .reset-all {
    flex: none;
    background: transparent;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.04em;
    padding: 0.25rem 0.55rem;
    cursor: pointer;
  }
  .reset-all:hover {
    border-color: var(--accent);
    color: var(--ink);
  }
</style>
