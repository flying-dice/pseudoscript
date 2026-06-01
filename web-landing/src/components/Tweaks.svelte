<script lang="ts">
  import { Settings2, X } from '@lucide/svelte';

  // Theme + motion. Theme follows the OS (prefers-color-scheme) live until the
  // user makes an explicit choice, which persists and wins thereafter. Initial
  // `data-theme` is resolved FOUC-free by the inline head script in index.html;
  // this panel reads it rather than forcing a default.
  type Theme = 'Dark' | 'Light';
  const STORE_KEY = 'pds-theme';

  function stored(): Theme | null {
    try {
      const v = localStorage.getItem(STORE_KEY);
      return v === 'dark' ? 'Dark' : v === 'light' ? 'Light' : null;
    } catch {
      return null;
    }
  }

  let open = $state<boolean>(false);
  let theme = $state<Theme>(
    document.documentElement.getAttribute('data-theme') === 'light' ? 'Light' : 'Dark',
  );
  // Once chosen explicitly, stop following the OS.
  let overridden = $state<boolean>(stored() !== null);
  let motion = $state<boolean>(!window.matchMedia('(prefers-reduced-motion: reduce)').matches);

  function applyTheme(next: Theme) {
    document.documentElement.setAttribute('data-theme', next === 'Light' ? 'light' : 'dark');
    theme = next;
  }

  function choose(next: Theme) {
    try {
      localStorage.setItem(STORE_KEY, next === 'Light' ? 'light' : 'dark');
    } catch {
      // private mode — choice holds for the session, just not across reloads
    }
    overridden = true;
    applyTheme(next);
  }

  // Follow prefers-color-scheme while unoverridden; when `overridden` flips true
  // the effect re-runs and its cleanup detaches the listener.
  $effect(() => {
    if (overridden) return;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const onChange = (e: MediaQueryListEvent) => applyTheme(e.matches ? 'Light' : 'Dark');
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  });

  $effect(() => {
    document.body.classList.toggle('no-motion', !motion);
  });
</script>

<div class="tweaks" class:open>
  {#if open}
    <div class="panel">
      <div class="ph">
        <span class="pl">Tweaks</span>
        <span class="sp"></span>
        <button class="x" onclick={() => (open = false)} aria-label="Close tweaks"><X size={14} /></button>
      </div>

      <div class="sec">Theme</div>
      <div class="seg-row">
        {#each ['Dark', 'Light'] as opt (opt)}
          <button class="seg grow" class:on={theme === opt} onclick={() => choose(opt as Theme)}>{opt}</button>
        {/each}
      </div>

      <div class="sec">Motion</div>
      <div class="seg-row">
        <button class="seg grow" class:on={motion} onclick={() => (motion = true)}>On</button>
        <button class="seg grow" class:on={!motion} onclick={() => (motion = false)}>Off</button>
      </div>
    </div>
  {/if}

  <button class="fab" onclick={() => (open = !open)} aria-label="Tweaks" aria-expanded={open}>
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
  /* keep clear of iOS home-indicator / Android nav bar */
  @supports (padding: max(0px)) {
    .tweaks {
      right: max(1.1rem, env(safe-area-inset-right));
      bottom: max(1.1rem, env(safe-area-inset-bottom));
    }
  }
  .fab {
    width: 44px;
    height: 44px;
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
    width: min(220px, calc(100vw - 2.2rem));
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
    width: 28px;
    height: 28px;
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
    padding: .45rem .7rem;
    min-height: 38px;
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
</style>
