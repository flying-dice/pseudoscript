<script>
  let {
    errorCount = 0,
    workspaceName = null,
    building = false,
    onformat,
    onproject,
    onbuilddocs,
    onopensettings,
  } = $props();
</script>

<header class="toolbar">
  <a class="brand" href="https://github.com/flying-dice/pseudoscript" target="_blank" rel="noreferrer" aria-label="PseudoScript Web IDE">
    <span class="dot" aria-hidden="true"></span>
    <span class="word">PseudoScript</span>
  </a>

  <button class="ghost project" onclick={onproject} title="Projects — recent &amp; examples">
    <span class="ico" aria-hidden="true">◳</span>
    {workspaceName ?? "Open project"}
    <span class="chev" aria-hidden="true">▾</span>
  </button>

  <div class="spacer"></div>

  <p class="hint">Hover a symbol for its diagram</p>

  <button class="ghost build" onclick={onbuilddocs} disabled={building} title="Build the static documentation site (pds doc)">
    <span class="ico" aria-hidden="true">⚙</span>
    {building ? "Building…" : "Build docs"}
  </button>

  <button
    class="ghost icon-only settings"
    onclick={onopensettings}
    title="Keyboard shortcuts"
    aria-label="Keyboard shortcuts"
  >
    <span class="ico" aria-hidden="true">⌨</span>
  </button>

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
  .ghost .chev { color: var(--ink-faint); font-size: 0.7rem; margin-left: 0.1rem; }
  .ghost:hover:not(:disabled) { border-color: var(--accent); color: var(--ink); }
  .ghost:hover:not(:disabled) .chev { color: var(--ink-soft); }
  .ghost:disabled { opacity: 0.45; cursor: not-allowed; }
  .icon-only { padding: 0.42rem 0.6rem; }
  .icon-only .ico { font-size: 0.95rem; }

  .spacer { flex: 1; }

  .hint {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    color: var(--ink-faint);
  }

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
