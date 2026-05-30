<script>
  let {
    view = $bindable("context"),
    target = $bindable(""),
    targets = [],
    errorCount = 0,
    workspaceName = null,
    canOpenFolder = false,
    onformat,
    onopenfolder,
  } = $props();

  // `sequence` is the compiler's view name; an app owner reads it as a "flow".
  const VIEWS = [
    { value: "context", label: "context" },
    { value: "container", label: "container" },
    { value: "component", label: "component" },
    { value: "sequence", label: "flow" },
  ];
  const needsTarget = $derived(view !== "context");
  const emptyLabel = $derived(
    view === "sequence" ? "no triggered callables" : view === "component" ? "no containers" : "no systems",
  );
</script>

<header class="toolbar">
  <a class="brand" href="https://c4model.com" target="_blank" rel="noreferrer" aria-label="PseudoScript Web IDE">
    <span class="dot" aria-hidden="true"></span>
    <span class="word">PseudoScript</span>
    <span class="eyebrow">C4 · WASM</span>
  </a>

  <button
    class="ghost open-folder"
    onclick={onopenfolder}
    disabled={!canOpenFolder}
    title={canOpenFolder
      ? "Open a folder as a workspace"
      : "Folder workspaces need a Chromium browser (File System Access API)"}
  >
    <span class="ico" aria-hidden="true">▢</span>
    {workspaceName ?? "Open folder"}
  </button>

  <div class="spacer"></div>

  <div class="controls">
    <label class="field">
      <span class="lbl">view</span>
      <select bind:value={view}>
        {#each VIEWS as v}<option value={v.value}>{v.label}</option>{/each}
      </select>
    </label>
    {#if needsTarget}
      <label class="field">
        <span class="lbl">target</span>
        {#if targets.length}
          <select bind:value={target}>
            {#each targets as t}<option value={t.fqn}>{t.name}</option>{/each}
          </select>
        {:else}
          <select disabled aria-label="no targets in this module">
            <option>{emptyLabel}</option>
          </select>
        {/if}
      </label>
    {/if}
  </div>

  <button class="format" onclick={onformat}>Format</button>

  <div class="status" class:bad={errorCount > 0} aria-live="polite">
    <span class="status-dot" class:bad={errorCount > 0}></span>
    {errorCount === 0 ? "no errors" : `${errorCount} error${errorCount === 1 ? "" : "s"}`}
  </div>
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    height: var(--topbar-h);
    padding: 0 1.1rem;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 78%, transparent);
    backdrop-filter: blur(10px) saturate(1.3);
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    color: inherit;
    text-decoration: none;
  }
  .brand .dot {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--accent);
    align-self: center;
    animation: pulse-dot 2.8s ease-out infinite;
  }
  .brand .word {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1.12rem;
    letter-spacing: -0.025em;
  }
  .brand .eyebrow {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    font-weight: 600;
    letter-spacing: 0.28em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    max-width: 18rem;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--ink-soft);
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    padding: 0.42rem 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  .ghost .ico { color: var(--accent); font-size: 0.8rem; }
  .ghost:hover:not(:disabled) { border-color: var(--accent); color: var(--ink); }
  .ghost:disabled { opacity: 0.45; cursor: not-allowed; }

  .spacer { flex: 1; }

  .controls {
    display: flex;
    align-items: stretch;
    gap: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0 0.6rem;
  }
  .field + .field { border-left: 1px solid var(--line-strong); }
  .field .lbl {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .field select {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    padding: 0.46rem 0.2rem;
    cursor: pointer;
  }
  .field select:disabled { color: var(--ink-faint); cursor: not-allowed; }
  .field select:focus { outline: none; color: var(--accent); }
  .field option { background: var(--surface-2); color: var(--ink); }

  .format {
    color: var(--accent-ink);
    background: var(--accent);
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.5rem 1rem;
    font-weight: 700;
    font-size: 0.85rem;
    letter-spacing: -0.01em;
    transition: background 0.14s, transform 0.14s;
  }
  .format:hover { background: var(--accent-hi); }
  .format:active { transform: translateY(1px); }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    min-width: 6rem;
    justify-content: flex-end;
    font-family: var(--font-mono);
    font-size: 0.74rem;
    color: var(--ok);
  }
  .status.bad { color: var(--err); }
  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
  }
  .status-dot.bad { background: var(--err); }
</style>
