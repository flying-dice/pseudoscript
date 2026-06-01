<script>
  // The project launcher: opens on start and from the toolbar. Recent projects
  // (samples + re-openable folders) on the left, the examples catalogue on the
  // right, "open a folder" below. Drafting-terminal styling to match the IDE.
  let {
    examples = [],
    recents = [],
    canOpenFolder = false,
    dismissible = false,
    onpicksample,
    onpickrecent,
    onopenfolder,
    onimport,
    onforget,
    onclose,
  } = $props();

  function ago(ts) {
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

  // Examples grouped by category, preserving the catalogue's sort order so the
  // list reads like a handbook table of contents (Application, Edge, Resilience,
  // Messaging, …).
  const grouped = $derived.by(() => {
    const order = [];
    const by = new Map();
    for (const ex of examples) {
      if (!by.has(ex.category)) {
        by.set(ex.category, []);
        order.push(ex.category);
      }
      by.get(ex.category).push(ex);
    }
    return order.map((cat) => ({ cat, items: by.get(cat) }));
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) close(); }}>
  <section class="dossier" role="dialog" aria-modal="true" aria-label="Open a project">
    <span class="tick tl"></span><span class="tick tr"></span><span class="tick bl"></span><span class="tick br"></span>

    <header class="head">
      <div class="brand">
        <svg class="logo" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <rect x="2.5" y="2.5" width="19" height="19" rx="4" stroke="currentColor" stroke-width="1.4" opacity="0.45" />
          <rect x="6.5" y="6.5" width="11" height="11" rx="2.6" stroke="currentColor" stroke-width="1.5" />
          <circle cx="12" cy="12" r="2.3" fill="var(--accent)" />
        </svg>
        <span class="word">PseudoScript</span>
        <span class="sep">/</span>
        <span class="eyebrow">Project</span>
      </div>
      {#if dismissible}
        <button class="x" onclick={close} aria-label="Close">✕</button>
      {/if}
    </header>

    <h1 class="title">Open a project</h1>
    <p class="lede">Architecture as code. Resume where you left off, explore a worked example, or open a folder of <code>.pds</code> modules.</p>

    <div class="grid">
      <div class="col start">
        <h2 class="kicker">Start</h2>
        <ul class="rows">
          <li>
            <button class="row action" data-testid="open-folder" onclick={() => onopenfolder?.()} disabled={!canOpenFolder}>
              <span class="glyph folder" aria-hidden="true">▢</span>
              <span class="meta">
                <span class="name">Open a folder</span>
                <span class="sub">a directory of <code>.pds</code> modules</span>
              </span>
              <span class="chev" aria-hidden="true">→</span>
            </button>
          </li>
          <li>
            <button class="row action" data-testid="import-workspace" onclick={() => onimport?.()}>
              <span class="glyph import" aria-hidden="true">↧</span>
              <span class="meta">
                <span class="name">Import a workspace</span>
                <span class="sub">a shared <code>.pdsx</code> file</span>
              </span>
              <span class="chev" aria-hidden="true">→</span>
            </button>
          </li>
        </ul>

        <h2 class="kicker">Recent</h2>
        {#if recents.length}
          <ul class="rows recents">
            {#each recents as r (r.key)}
              <li>
                <button class="row" onclick={() => onpickrecent?.(r)}>
                  <span class="glyph {r.kind}" aria-hidden="true">{r.kind === "sample" ? "▣" : "▢"}</span>
                  <span class="meta">
                    <span class="name">{r.name}</span>
                    <span class="sub">{r.kind === "sample" ? "example" : "folder"} · {ago(r.at)}</span>
                  </span>
                </button>
                <button class="forget" title="Remove from recent" aria-label="Remove {r.name} from recent" onclick={() => onforget?.(r)}>✕</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="empty">No recent projects yet — open an example to begin.</p>
        {/if}

        {#if !canOpenFolder}
          <p class="note">Local folders need a Chromium browser (File System Access API). Examples and import work everywhere.</p>
        {/if}
      </div>

      <div class="col examples">
        <h2 class="kicker">Examples</h2>
        {#each grouped as group (group.cat)}
          <h3 class="group">{group.cat}</h3>
          <ul class="cards">
            {#each group.items as ex (ex.id)}
              <li>
                <button class="card" data-testid="sample-{ex.id}" onclick={() => onpicksample?.(ex.id)}>
                  <span class="ct tl"></span><span class="ct br"></span>
                  <span class="card-top">
                    <span class="card-name">{ex.name}</span>
                    <span class="count">{ex.moduleCount} module{ex.moduleCount === 1 ? "" : "s"}</span>
                  </span>
                  <span class="desc">{ex.description}</span>
                  <span class="go">Open <span class="arr" aria-hidden="true">→</span></span>
                </button>
              </li>
            {/each}
          </ul>
        {/each}
      </div>
    </div>
  </section>
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
    width: min(64rem, 100%);
    max-height: calc(100vh - 4rem);
    /* hidden, not auto: hover nudges (.arr/.card transforms) and the staggered
       rise entrance momentarily extend the box and would flash a scrollbar.
       Content is bounded (≤8 recents + example cards) so nothing is clipped. */
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

  /* drafting corner ticks */
  .tick {
    position: absolute;
    width: 13px;
    height: 13px;
    border: 1.5px solid var(--accent);
    opacity: 0.7;
    pointer-events: none;
  }
  .tick.tl { top: -1px; left: -1px; border-right: 0; border-bottom: 0; }
  .tick.tr { top: -1px; right: -1px; border-left: 0; border-bottom: 0; }
  .tick.bl { bottom: -1px; left: -1px; border-right: 0; border-top: 0; }
  .tick.br { bottom: -1px; right: -1px; border-left: 0; border-top: 0; }

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
  .brand .sep { color: var(--ink-faint); }
  .brand .eyebrow {
    font-family: var(--font-mono); font-size: 0.66rem; letter-spacing: 0.22em;
    text-transform: uppercase; color: var(--accent);
  }
  .x {
    width: 2rem; height: 2rem; display: grid; place-items: center;
    background: var(--surface-2); border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm); color: var(--ink-soft); font-size: 0.85rem;
  }
  .x:hover { border-color: var(--accent); color: var(--ink); }

  .title {
    margin: 1.2rem 0 0.4rem;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: clamp(1.8rem, 3.4vw, 2.5rem);
    letter-spacing: -0.035em;
    line-height: 1.02;
    animation: rise 0.4s 0.06s both;
  }
  .lede {
    margin: 0 0 1.7rem;
    max-width: 44rem;
    color: var(--ink-soft);
    line-height: 1.55;
    animation: rise 0.4s 0.1s both;
  }
  .lede code { font-family: var(--font-mono); font-size: 0.86em; color: var(--ink); }

  .grid {
    display: grid;
    grid-template-columns: minmax(0, 13fr) minmax(0, 17fr);
    gap: 1.6rem;
  }
  @media (max-width: 720px) { .grid { grid-template-columns: 1fr; } }

  .kicker {
    margin: 0 0 0.85rem;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    display: flex; align-items: center; gap: 0.6rem;
  }
  .kicker::after { content: ""; flex: 1; height: 1px; background: var(--line); }

  .col.start { animation: rise 0.4s 0.14s both; display: flex; flex-direction: column; }
  .col.start .kicker:not(:first-child) { margin-top: 1.4rem; }
  /* the catalogue can run long (the patterns handbook), so it scrolls inside the
     dossier while the header and recent column stay put. */
  .col.examples {
    animation: rise 0.4s 0.18s both;
    min-height: 0;
    max-height: 64vh;
    overflow-y: auto;
    padding-right: 0.4rem;
  }

  /* category subheading within the examples column (the handbook's sections) */
  .group {
    margin: 1.1rem 0 0.55rem;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-soft);
  }
  .group:first-of-type { margin-top: 0; }

  /* one row language for both Start actions and Recent entries */
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
  .row.action { background: var(--surface-2); border-color: var(--line); }
  .row.action:hover:not(:disabled) { border-color: var(--accent); }
  .row.action:disabled { opacity: 0.45; cursor: not-allowed; }
  .row .chev {
    margin-left: auto; flex: none; color: var(--accent);
    font-family: var(--font-mono); opacity: 0; transform: translateX(-3px);
    transition: opacity 0.15s, transform 0.15s;
  }
  .row.action:hover:not(:disabled) .chev { opacity: 1; transform: none; }
  .glyph {
    flex: none; width: 1.9rem; height: 1.9rem; display: grid; place-items: center;
    font-family: var(--font-mono); font-size: 0.9rem;
    border: 1px solid var(--line-strong); border-radius: 6px;
    background: var(--surface);
  }
  .glyph.sample { color: var(--accent); }
  .glyph.folder { color: var(--k-container); }
  .glyph.import { color: var(--k-person); }
  .meta { display: flex; flex-direction: column; min-width: 0; }
  .meta .name { font-weight: 600; font-size: 0.92rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta .sub { font-family: var(--font-mono); font-size: 0.66rem; color: var(--ink-faint); }
  .meta .sub code { font-size: 0.95em; color: var(--ink-soft); }
  .recents { margin-top: 0; }
  .forget {
    position: absolute; right: 0.35rem; top: 50%; transform: translateY(-50%);
    width: 1.5rem; height: 1.5rem; display: grid; place-items: center;
    background: var(--surface-3); border: 1px solid var(--line-strong); border-radius: 5px;
    color: var(--ink-faint); font-size: 0.66rem; opacity: 0;
  }
  .recents li:hover .forget { opacity: 1; }
  .forget:hover { color: var(--err); border-color: var(--err); }

  .empty { margin: 0.2rem 0 0; color: var(--ink-faint); font-size: 0.85rem; line-height: 1.5; }
  .note { margin: 1.1rem 0 0; font-size: 0.72rem; color: var(--ink-faint); line-height: 1.45; }

  .cards { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr)); gap: 0.7rem; }
  .card {
    position: relative; width: 100%; height: 100%;
    display: flex; flex-direction: column; gap: 0.5rem;
    text-align: left;
    padding: 1rem 1.05rem 0.9rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    transition: border-color 0.15s, transform 0.15s, background 0.15s;
  }
  .card:hover {
    border-color: var(--accent);
    transform: translateY(-2px);
    background: color-mix(in srgb, var(--surface-2) 84%, var(--accent) 8%);
  }
  /* card corner ticks (top-left, bottom-right), revealed on hover */
  .ct { position: absolute; width: 9px; height: 9px; border: 1.5px solid var(--accent); opacity: 0; transition: opacity 0.15s; }
  .ct.tl { top: 5px; left: 5px; border-right: 0; border-bottom: 0; }
  .ct.br { bottom: 5px; right: 5px; border-left: 0; border-top: 0; }
  .card:hover .ct { opacity: 0.75; }
  .card-top { display: flex; align-items: baseline; justify-content: space-between; gap: 0.6rem; }
  .card-name { font-family: var(--font-display); font-weight: 700; font-size: 1.08rem; letter-spacing: -0.02em; }
  .count { flex: none; font-family: var(--font-mono); font-size: 0.64rem; color: var(--ink-faint); }
  .desc { color: var(--ink-soft); font-size: 0.84rem; line-height: 1.5; }
  .go {
    margin-top: 0.15rem; font-family: var(--font-mono); font-size: 0.68rem; letter-spacing: 0.08em;
    text-transform: uppercase; color: var(--accent); display: inline-flex; align-items: center; gap: 0.4rem;
  }
  .arr { transition: transform 0.15s; }
  .card:hover .arr { transform: translateX(3px); }
</style>
