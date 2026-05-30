<script>
  let { diagnostics = [] } = $props();
</script>

<div class="problems">
  {#if diagnostics.length === 0}
    <div class="empty">No problems — the model is well-formed.</div>
  {:else}
    <ul>
      {#each diagnostics as d}
        <li class={d.severity}>
          <span class="badge">{d.severity}</span>
          <span class="loc">{d.start_line}:{d.start_col}</span>
          <span class="msg">{d.message}</span>
          {#if d.code}<span class="code">{d.code}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .problems {
    height: 100%;
    overflow: auto;
    background: var(--surface);
  }
  .empty {
    padding: 1rem 1.1rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--ok);
  }
  ul { list-style: none; margin: 0; padding: 0.3rem 0; }
  li {
    display: grid;
    grid-template-columns: auto auto 1fr auto;
    align-items: baseline;
    gap: 0.6rem;
    padding: 0.4rem 1.1rem;
    font-size: 0.82rem;
    border-bottom: 1px solid var(--line);
  }
  .badge {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0.12rem 0.4rem;
    border-radius: 4px;
  }
  li.error .badge { color: var(--err); background: rgba(255, 90, 82, 0.14); }
  li.warning .badge { color: var(--warn); background: rgba(224, 169, 63, 0.14); }
  li.info .badge { color: var(--ink-soft); background: var(--surface-2); }
  .loc { font-family: var(--font-mono); font-size: 0.72rem; color: var(--ink-faint); }
  .msg { color: var(--ink); }
  .code { font-family: var(--font-mono); font-size: 0.72rem; color: var(--ink-faint); }
</style>
