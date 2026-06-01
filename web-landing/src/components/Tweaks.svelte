<script lang="ts">
  import { Settings2, X } from '@lucide/svelte';

  type Accent = readonly [string, string];

  // Theme · accent · motion — mirrors the design's React tweaks island, but
  // self-contained. Applies values to the document root; the page reacts via
  // CSS tokens / the body.no-motion class.
  const ACCENTS: readonly Accent[] = [
    ['#ff5a36', '#ff7d5e'],
    ['#6e8bff', '#8aa2ff'],
    ['#2dd4bf', '#5fe6d4'],
    ['#e0a93f', '#ecc06a'],
  ];

  let open = $state<boolean>(false);
  let theme = $state<string>('Dark');
  let accent = $state<Accent>(ACCENTS[0]);
  let motion = $state<boolean>(true);

  $effect(() => {
    document.documentElement.setAttribute('data-theme', theme === 'Light' ? 'light' : 'dark');
  });

  $effect(() => {
    const root = document.documentElement.style;
    if (accent && accent[0] && accent[0].toLowerCase() !== '#ff5a36') {
      root.setProperty('--accent', accent[0]);
      root.setProperty('--accent-hi', accent[1] || accent[0]);
    } else {
      // default vermilion — let the theme token decide (keeps light-mode accent)
      root.removeProperty('--accent');
      root.removeProperty('--accent-hi');
    }
  });

  $effect(() => {
    document.body.classList.toggle('no-motion', !motion);
  });
</script>

<div class="tweaks" class:open>
  {#if open}
    <div class="panel ticked">
      <span class="tick tl"></span><span class="tick br"></span>
      <div class="ph">
        <span class="pl">Tweaks</span>
        <span class="sp"></span>
        <button class="x" onclick={() => (open = false)} aria-label="Close tweaks"><X size={14} /></button>
      </div>

      <div class="sec">Theme</div>
      <div class="seg-row">
        {#each ['Dark', 'Light'] as opt}
          <button class="seg" class:on={theme === opt} onclick={() => (theme = opt)}>{opt}</button>
        {/each}
      </div>

      <div class="sec">Accent</div>
      <div class="swatches">
        {#each ACCENTS as a}
          <button
            class="sw"
            class:on={accent[0] === a[0]}
            style="--sw:{a[0]}"
            onclick={() => (accent = a)}
            aria-label={`Accent ${a[0]}`}
          ></button>
        {/each}
      </div>

      <div class="sec">Motion</div>
      <div class="seg-row">
        <button class="seg grow" class:on={motion} onclick={() => (motion = true)}>On</button>
        <button class="seg grow" class:on={!motion} onclick={() => (motion = false)}>Off</button>
      </div>
    </div>
  {/if}

  <button class="fab" onclick={() => (open = !open)} aria-label="Tweaks">
    <Settings2 size={18} strokeWidth={1.75} />
  </button>
</div>

<style>
  .tweaks {
    position: fixed;
    right: 1.1rem;
    bottom: 1.1rem;
    z-index: 200;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: .7rem;
    font-family: var(--font-sans);
  }
  .fab {
    width: 42px;
    height: 42px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
    background: var(--surface-2);
    color: var(--ink-soft);
    box-shadow: var(--shadow-md);
    cursor: pointer;
    transition: color .14s, border-color .14s, transform .14s;
  }
  .fab:hover { color: var(--accent); border-color: var(--accent); transform: translateY(-1px); }

  .panel {
    width: 220px;
    padding: .9rem 1rem 1rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }
  .ph { display: flex; align-items: center; margin-bottom: .8rem; }
  .ph .pl {
    font-family: var(--font-mono);
    font-size: .6rem;
    font-weight: 600;
    letter-spacing: .2em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .ph .sp { flex: 1; }
  .ph .x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: none;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .ph .x:hover { color: var(--ink); background: var(--surface-2); }

  .sec {
    font-family: var(--font-mono);
    font-size: .56rem;
    font-weight: 600;
    letter-spacing: .2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    margin: .9rem 0 .45rem;
  }
  .sec:first-of-type { margin-top: 0; }

  .seg-row { display: flex; gap: .4rem; }
  .seg {
    flex: none;
    padding: .35rem .7rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: .68rem;
    cursor: pointer;
    transition: color .14s, border-color .14s, background .14s;
  }
  .seg.grow { flex: 1; }
  .seg:hover { color: var(--ink); border-color: var(--ink-faint); }
  .seg.on {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .swatches { display: flex; gap: .5rem; }
  .sw {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    background: var(--sw);
    cursor: pointer;
    padding: 0;
    transition: transform .14s, border-color .14s;
  }
  .sw:hover { transform: scale(1.08); }
  .sw.on { border-color: var(--ink); transform: scale(1.08); }
</style>
