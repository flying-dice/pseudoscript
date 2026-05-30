<script>
  let {
    view = $bindable("context"),
    target = $bindable(""),
    errorCount = 0,
    onformat,
  } = $props();

  const VIEWS = ["context", "container", "component", "sequence"];
  const needsTarget = $derived(view !== "context");
</script>

<header class="toolbar">
  <div class="brand">
    <span class="dot"></span>
    <span class="name">PseudoScript</span>
    <span class="sub">Web IDE</span>
  </div>

  <div class="spacer"></div>

  <div class="diagram-controls">
    <label class="field">
      <span>view</span>
      <select bind:value={view}>
        {#each VIEWS as v}<option value={v}>{v}</option>{/each}
      </select>
    </label>
    {#if needsTarget}
      <label class="field">
        <span>target</span>
        <input
          type="text"
          bind:value={target}
          placeholder="shop::Storefront"
          spellcheck="false"
          autocomplete="off"
        />
      </label>
    {/if}
  </div>

  <button class="format" onclick={onformat}>Format</button>

  <div class="status" class:bad={errorCount > 0}>
    {errorCount === 0 ? "no errors" : `${errorCount} error${errorCount === 1 ? "" : "s"}`}
  </div>
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    height: var(--topbar-h);
    padding: 0 1rem;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 85%, transparent);
    backdrop-filter: blur(8px);
  }
  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  .brand .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    align-self: center;
    box-shadow: 0 0 0 4px var(--accent-soft);
  }
  .brand .name {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .brand .sub {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .spacer { flex: 1; }
  .diagram-controls {
    display: flex;
    gap: 0.6rem;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .field select,
  .field input {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-transform: none;
    letter-spacing: 0;
    color: var(--ink);
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    padding: 0.3rem 0.5rem;
  }
  .field input { width: 12rem; }
  .field select:focus,
  .field input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .format {
    color: var(--accent-ink);
    background: var(--accent);
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.45rem 0.9rem;
    font-weight: 600;
    transition: background 0.13s;
  }
  .format:hover { background: var(--accent-hi); }
  .status {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ok);
    min-width: 5.5rem;
    text-align: right;
  }
  .status.bad { color: var(--err); }
</style>
