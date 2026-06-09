<script lang="ts">
  import type { Command } from "$lib/keybindings.svelte.js";
  import { COMMANDS, PROFILES, chordFromEvent, formatChord, keybindings } from "$lib/keybindings.svelte.js";
  import { llm } from "$lib/llm.svelte.js";

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
        Ghost-text suggestions from any OpenAI-compatible endpoint — a local Ollama, or a hosted
        provider with your own key. Grammar autocomplete stays local either way.
      </p>

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
      <div class="field">
        <label for="llm-apikey">API key</label>
        <input
          id="llm-apikey"
          type="password"
          autocomplete="off"
          data-testid="llm-apikey"
          placeholder="empty for a local model"
          value={llm.apiKey}
          oninput={(e) => llm.set({ apiKey: e.currentTarget.value })}
        />
        <p class="hint-line">Stored in this browser only; sent only to the endpoint above.</p>
      </div>
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
