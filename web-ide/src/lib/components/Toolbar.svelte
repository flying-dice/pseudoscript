<script>
  import { theme, THEME_OPTIONS } from "$lib/theme.svelte.js";

  let {
    errorCount = 0,
    workspaceName = null,
    building = false,
    // Save state for the workspace indicator: number of files differing from
    // disk, whether the workspace can persist (a real folder vs a sample), and
    // the active write lifecycle (idle | saving | saved | error).
    dirtyCount = 0,
    canPersist = false,
    saveState = "idle",
    onformat,
    onproject,
    onbuilddocs,
    onshare,
    onexport,
    onimport,
    onopensettings,
  } = $props();

  // The share/transport overflow menu (Share link · Export · Import).
  let shareOpen = $state(false);

  // Theme: cycle system → light → dark. The glyph shows the current preference.
  const THEME_ICON = { system: "◐", light: "☀", dark: "☾" };
  const THEME_LABEL = { system: "System", light: "Light", dark: "Dark" };
  function cycleTheme() {
    const i = THEME_OPTIONS.indexOf(theme.pref);
    theme.set(THEME_OPTIONS[(i + 1) % THEME_OPTIONS.length]);
  }
</script>

<header class="toolbar">
  <a class="brand" href="https://github.com/flying-dice/pseudoscript" target="_blank" rel="noreferrer" aria-label="PseudoScript Web IDE">
    <svg class="logo" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="2.5" y="2.5" width="19" height="19" rx="4" stroke="currentColor" stroke-width="1.4" opacity="0.45" />
      <rect x="6.5" y="6.5" width="11" height="11" rx="2.6" stroke="currentColor" stroke-width="1.5" />
      <circle cx="12" cy="12" r="2.3" fill="var(--accent)" />
    </svg>
    <span class="word">PseudoScript</span>
  </a>

  <button class="ghost project" onclick={onproject} title="Projects — recent &amp; examples">
    <span class="ico" aria-hidden="true">◳</span>
    {workspaceName ?? "Open project"}
    <span class="chev" aria-hidden="true">▾</span>
  </button>

  <div class="spacer"></div>

  <p class="hint">Hover a symbol for its diagram</p>

  <div class="share-wrap">
    <button
      class="ghost share"
      class:open={shareOpen}
      onclick={() => (shareOpen = !shareOpen)}
      title="Share, export, or import a workspace"
      aria-haspopup="menu"
      aria-expanded={shareOpen}
    >
      <span class="ico" aria-hidden="true">↗</span>
      Share
      <span class="chev" aria-hidden="true">▾</span>
    </button>
    {#if shareOpen}
      <button class="share-scrim" aria-label="Close" onclick={() => (shareOpen = false)}></button>
      <div class="share-menu" role="menu">
        <button role="menuitem" disabled={!workspaceName} onclick={() => { shareOpen = false; onshare?.(); }}>
          <span class="mi-title">Copy share link</span>
          <span class="mi-sub">Whole workspace, in the URL</span>
        </button>
        <button role="menuitem" disabled={!workspaceName} onclick={() => { shareOpen = false; onexport?.(); }}>
          <span class="mi-title">Export <code>.pdsx</code></span>
          <span class="mi-sub">Download a compressed file</span>
        </button>
        <button role="menuitem" onclick={() => { shareOpen = false; onimport?.(); }}>
          <span class="mi-title">Import <code>.pdsx</code>…</span>
          <span class="mi-sub">Open an exported workspace</span>
        </button>
      </div>
    {/if}
  </div>

  <button class="ghost build" onclick={onbuilddocs} disabled={building} title="Build the static documentation site (pds doc)">
    <span class="ico" aria-hidden="true">⚙</span>
    {building ? "Building…" : "Build docs"}
  </button>

  <button
    class="ghost icon-only theme"
    onclick={cycleTheme}
    title={`Theme: ${THEME_LABEL[theme.pref]} — click to change`}
    aria-label={`Theme: ${THEME_LABEL[theme.pref]}`}
  >
    <span class="ico" aria-hidden="true">{THEME_ICON[theme.pref]}</span>
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

  {#if workspaceName}
    <div class="save" aria-live="polite">
      {#if !canPersist}
        {#if dirtyCount > 0}
          <span class="save-dot warn"></span><span class="save-label warn" title="In-memory session — Save to a folder to keep these edits">session · {dirtyCount} unsaved</span>
        {:else}
          <span class="save-dot dim"></span><span class="save-label dim" title="Bundled example — edits live in memory only">session</span>
        {/if}
      {:else if saveState === "saving"}
        <span class="save-dot busy"></span><span class="save-label">saving…</span>
      {:else if saveState === "error"}
        <span class="save-dot bad"></span><span class="save-label bad">save failed</span>
      {:else if dirtyCount > 0}
        <span class="save-dot warn"></span><span class="save-label warn">{dirtyCount} unsaved</span>
      {:else}
        <span class="save-dot ok"></span><span class="save-label">saved</span>
      {/if}
    </div>
  {/if}

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
    /* `backdrop-filter` makes the toolbar a stacking context, so the share
       dropdown's z-index is local to it. Lift the whole toolbar above the
       workspace panes (z-index ≤ 41) — but below modals (≥ 50) — so the
       dropdown isn't painted over by the main body. */
    position: relative;
    z-index: 45;
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    color: inherit;
    text-decoration: none;
  }
  .brand .logo {
    width: 22px;
    height: 22px;
    align-self: center;
    color: var(--ink-soft);
    transition: color 0.16s, transform 0.3s cubic-bezier(0.2, 0.7, 0.2, 1);
  }
  .brand:hover .logo { color: var(--ink); transform: rotate(-90deg); }
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

  .save {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink-soft);
  }
  .save-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
  }
  .save-dot.warn { background: var(--warn); }
  .save-dot.bad { background: var(--err); }
  .save-dot.dim { background: var(--ink-faint); }
  .save-dot.busy { background: var(--ink-faint); animation: pulse-dot 1.2s ease-in-out infinite; }
  .save-label.dim { color: var(--ink-faint); }
  .save-label.warn { color: var(--warn); }
  .save-label.bad { color: var(--err); }

  /* share / export / import overflow menu */
  .share-wrap { position: relative; }
  .share.open { border-color: var(--accent); color: var(--ink); }
  .share-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: none;
  }
  .share-menu {
    position: absolute;
    top: calc(100% + 0.4rem);
    left: 0;
    z-index: 41;
    width: 16rem;
    display: flex;
    flex-direction: column;
    padding: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg, 0 18px 50px rgba(0, 0, 0, 0.5));
  }
  .share-menu button {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    text-align: left;
    padding: 0.45rem 0.6rem;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink);
  }
  .share-menu button:hover:not(:disabled) { background: var(--surface-2); }
  .share-menu button:disabled { opacity: 0.4; cursor: not-allowed; }
  .share-menu .mi-title { font-size: 0.82rem; font-weight: 600; }
  .share-menu .mi-title code {
    font-family: var(--font-mono);
    font-size: 0.78em;
    color: var(--accent);
  }
  .share-menu .mi-sub { font-size: 0.72rem; color: var(--ink-faint); }
</style>
