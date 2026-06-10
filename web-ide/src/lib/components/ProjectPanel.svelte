<script lang="ts">
  // The project launcher: opens on start and from the toolbar. Open one of the
  // bundled examples in memory (no disk, no commitment — model:
  // ide::Launcher.openExample), open a folder, re-open a recent, or start a New
  // project (which opens its own dialog). Only the disk actions gate on File
  // System Access support. Drafting styling.
  import type { Recent } from "$lib/recents";
  import { FS_ACCESS_BROWSERS } from "$lib/workspace.js";

  type Example = { id: string; name: string; description: string; moduleCount: number };

  type Props = {
    recents?: Recent[];
    examples?: Example[];
    fsSupported?: boolean;
    dismissible?: boolean;
    onpickrecent?: (recent: Recent) => void;
    onopenfolder?: () => void;
    onnewproject?: () => void;
    onpickexample?: (id: string) => void;
    onforget?: (recent: Recent) => void;
    onclose?: () => void;
  };

  let {
    recents = [],
    examples = [],
    fsSupported = true,
    dismissible = false,
    onpickrecent,
    onopenfolder,
    onnewproject,
    onpickexample,
    onforget,
    onclose,
  }: Props = $props();

  // Only folder recents re-open (examples are templates now, not session mounts).
  const folderRecents = $derived(recents.filter((r) => r.kind !== "sample"));

  // The recents filter (IntelliJ-style search-as-you-type over project names).
  let query = $state("");
  const filtered = $derived(
    query.trim() ? folderRecents.filter((r) => r.name.toLowerCase().includes(query.trim().toLowerCase())) : folderRecents,
  );

  // The project avatar: up to two initials from the name's words (or its first
  // two letters), on a colour derived from a stable hash of the name — so each
  // project keeps the same avatar across sessions (IntelliJ-style).
  function initials(name: string): string {
    const parts = name.split(/[-_.\s]+/).filter(Boolean);
    if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
    return (parts[0] ?? name).slice(0, 2).toUpperCase();
  }
  function hue(name: string): number {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
    return h;
  }

  function ago(ts: number): string {
    const s = Math.max(1, Math.round((Date.now() - ts) / 1000));
    if (s < 60) return "just now";
    const m = Math.round(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h}h ago`;
    const d = Math.round(h / 24);
    if (d < 7) return `${d}d ago`;
    return new Date(ts).toLocaleDateString();
  }

  const close = () => dismissible && onclose?.();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) close(); }}>
  <div class="dossier" role="dialog" aria-modal="true" aria-label="Open a project">
    <header class="head">
      <div class="brand">
        <svg class="logo" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <rect x="2.5" y="2.5" width="19" height="19" rx="4" stroke="currentColor" stroke-width="1.4" opacity="0.45" />
          <rect x="6.5" y="6.5" width="11" height="11" rx="2.6" stroke="currentColor" stroke-width="1.5" />
          <circle cx="12" cy="12" r="2.3" fill="var(--accent)" />
        </svg>
        <span class="word">PseudoScript</span>
      </div>
      {#if dismissible}
        <button class="x" onclick={close} aria-label="Close">✕</button>
      {/if}
    </header>

    <div class="toolbar">
      <label class="search">
        <span class="search-glyph" aria-hidden="true">⌕</span>
        <input
          class="search-input"
          data-testid="search-projects"
          bind:value={query}
          placeholder="Search projects"
          aria-label="Search projects"
        />
      </label>
      <button
        class="btn"
        data-testid="open-folder"
        disabled={!fsSupported}
        title={fsSupported ? "Open a project folder from disk" : `Folders need the File System Access API (${FS_ACCESS_BROWSERS})`}
        onclick={() => onopenfolder?.()}>Open folder</button
      >
      <button
        class="btn primary"
        data-testid="new-project"
        disabled={!fsSupported}
        title={fsSupported ? undefined : `New projects write to disk, which needs the File System Access API (${FS_ACCESS_BROWSERS})`}
        onclick={() => onnewproject?.()}>New Project</button
      >
    </div>

    {#if !fsSupported}
      <p class="fs-note" data-testid="fs-note">
        This browser has no File System Access API, so folders can't be opened or saved — that needs
        {FS_ACCESS_BROWSERS}. The examples below still open right here, in memory.
      </p>
    {/if}

    <div class="recent">
      {#if filtered.length}
        <ul class="rows recents">
          {#each filtered as r (r.key)}
            <li>
              <button class="row" data-testid="recent-{r.key}" onclick={() => onpickrecent?.(r)}>
                <span class="avatar" style="--h: {hue(r.name)}" aria-hidden="true">{initials(r.name)}</span>
                <span class="meta">
                  <span class="name">{r.name}</span>
                  <span class="sub">{r.dir ?? r.name} · {ago(r.at)}</span>
                </span>
              </button>
              <button class="forget" title="Remove from recent" aria-label="Remove {r.name} from recent" onclick={() => onforget?.(r)}>✕</button>
            </li>
          {/each}
        </ul>
      {:else if folderRecents.length}
        <p class="empty">No projects match “{query.trim()}”.</p>
      {:else if fsSupported}
        <p class="empty">No recent projects yet — open an example below, or create one with New Project.</p>
      {/if}

      {#if examples.length}
        <h2 class="kicker">Examples — open in your browser, nothing to install</h2>
        <ul class="rows examples">
          {#each examples as ex (ex.id)}
            <li>
              <button class="row" data-testid="example-{ex.id}" onclick={() => onpickexample?.(ex.id)}>
                <span class="avatar" style="--h: {hue(ex.name)}" aria-hidden="true">{initials(ex.name)}</span>
                <span class="meta">
                  <span class="name">{ex.name}</span>
                  <span class="sub">{ex.moduleCount} module{ex.moduleCount === 1 ? "" : "s"} · opens in memory</span>
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
    padding: 2rem;
    background: color-mix(in srgb, var(--bg) 78%, transparent);
    backdrop-filter: blur(7px) saturate(1.1);
    animation: fade 0.18s ease-out;
  }
  @keyframes fade { from { opacity: 0; } to { opacity: 1; } }

  .dossier {
    position: relative;
    width: min(52rem, 100%);
    max-height: calc(100vh - 4rem);
    /* hidden, not auto: the staggered rise entrance momentarily extends the box
       and would flash a scrollbar; the recents list scrolls internally. */
    overflow: hidden;
    padding: 1.9rem 2rem 2rem;
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--surface) 96%, transparent), color-mix(in srgb, var(--surface) 88%, transparent)),
      var(--glow);
    background-color: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg), 0 0 0 1px var(--line);
    animation: dossier-in 0.34s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes dossier-in {
    from { opacity: 0; transform: translateY(14px) scale(0.985); }
    to { opacity: 1; transform: none; }
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    animation: rise 0.4s 0.02s both;
  }
  .brand { display: flex; align-items: baseline; gap: 0.55rem; }
  .brand .logo {
    width: 20px; height: 20px; align-self: center;
    color: var(--ink-soft);
  }
  .brand .word { font-family: var(--font-display); font-weight: 700; font-size: 1.04rem; letter-spacing: -0.025em; }
  .x {
    width: 1.75rem; height: 1.75rem; display: grid; place-items: center;
    background: transparent; border: none;
    border-radius: var(--radius-sm); color: var(--ink-faint); font-size: 0.9rem;
    transition: background 0.12s, color 0.12s;
  }
  .x:hover { background: var(--surface-2); color: var(--ink); }

  /* The IntelliJ-style toolbar: a search field that filters recents, with the
     Open and New Project actions to its right. */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 1.4rem 0 1.1rem;
    animation: rise 0.4s 0.06s both;
  }
  .search {
    flex: 1; min-width: 0;
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0 0.65rem;
    height: 2.1rem;
    background: var(--surface); border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
  }
  .search:focus-within { border-color: var(--accent); }
  .search-glyph { flex: none; color: var(--ink-faint); font-size: 0.95rem; }
  .search-input {
    flex: 1; min-width: 0;
    background: transparent; border: none; outline: none;
    color: var(--ink); font-family: var(--font-sans); font-size: 0.9rem;
  }
  .search-input::placeholder { color: var(--ink-faint); }
  .btn {
    flex: none;
    height: 2.1rem; padding: 0 0.95rem;
    background: var(--surface-2); border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink); font-family: var(--font-sans); font-size: 0.85rem; font-weight: 600;
    white-space: nowrap;
  }
  .btn:hover:not(:disabled) { border-color: var(--accent); }
  .btn.primary { background: var(--accent); border-color: var(--accent); color: var(--bg); }
  .btn.primary:hover:not(:disabled) { filter: brightness(1.06); }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }

  /* the unsupported-browser note: disk actions are off, examples still work */
  .fs-note {
    margin: -0.4rem 0 0.9rem;
    font-size: 0.8rem;
    line-height: 1.5;
    color: var(--ink-soft);
  }

  .kicker {
    margin: 1.1rem 0 0.5rem;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
    display: flex; align-items: center; gap: 0.6rem;
  }
  .kicker::after { content: ""; flex: 1; height: 1px; background: var(--line); }

  .recent {
    animation: rise 0.4s 0.1s both;
    min-height: 0; max-height: 64vh; overflow-y: auto;
  }

  .rows { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; }
  .rows li { position: relative; display: flex; align-items: stretch; }
  .row {
    flex: 1;
    display: flex; align-items: center; gap: 0.7rem;
    text-align: left;
    padding: 0.55rem 0.6rem;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
  }
  .row:hover:not(:disabled) { background: var(--surface-2); border-color: var(--line-strong); }
  /* The project avatar: initials on a per-project hue (set via the --h var). */
  .avatar {
    flex: none; width: 2.1rem; height: 2.1rem; display: grid; place-items: center;
    border-radius: 8px;
    font-family: var(--font-display); font-weight: 700; font-size: 0.8rem; letter-spacing: 0.01em;
    color: hsl(var(--h) 65% 88%);
    background: linear-gradient(150deg, hsl(var(--h) 45% 38%), hsl(var(--h) 50% 28%));
    box-shadow: inset 0 0 0 1px hsl(var(--h) 40% 50% / 0.5);
  }
  .meta { display: flex; flex-direction: column; min-width: 0; }
  .meta .name { font-weight: 600; font-size: 0.92rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta .sub { font-family: var(--font-mono); font-size: 0.66rem; color: var(--ink-faint); }
  .recents { margin-top: 0; }
  .forget {
    position: absolute; right: 0.35rem; top: 50%; transform: translateY(-50%);
    width: 1.5rem; height: 1.5rem; display: grid; place-items: center;
    background: transparent; border: none; border-radius: var(--radius-sm);
    color: var(--ink-faint); font-size: 0.7rem; opacity: 0;
    transition: background 0.12s, color 0.12s;
  }
  .recents li:hover .forget { opacity: 1; }
  .forget:hover { background: var(--surface-3); color: var(--err); }

  .empty { margin: 0.2rem 0 0; color: var(--ink-faint); font-size: 0.85rem; line-height: 1.5; }
</style>
