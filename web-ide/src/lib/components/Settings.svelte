<script lang="ts">
  import type { Command } from "$lib/keybindings.svelte.js";
  import { COMMANDS, PROFILES, chordFromEvent, formatChord, keybindings } from "$lib/keybindings.svelte.js";

  type Props = {
    onclose: () => void;
  };

  let { onclose }: Props = $props();

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
<div class="panel" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
  <header>
    <h2>Keyboard shortcuts</h2>
    <button class="x" aria-label="Close" onclick={onclose}>✕</button>
  </header>

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
                  <button class="chord recording" onclick={stopRecording}>Press keys… (Esc)</button>
                {:else}
                  <button class="chord" class:custom={cmd.custom} onclick={() => startRecording(cmd.id)}>
                    {formatChord(cmd.chord)}
                  </button>
                {/if}
                <button
                  class="reset"
                  title="Reset to default"
                  disabled={!cmd.custom}
                  onclick={() => keybindings.reset(cmd.id)}>↺</button
                >
              </div>
              {#if recording === cmd.id && notice}
                <p class="notice">{notice}</p>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  <footer>
    <span class="hint">Click a shortcut, then press the new key combination.</span>
    <button class="reset-all" onclick={() => keybindings.resetAll()}>Reset all</button>
  </footer>
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
