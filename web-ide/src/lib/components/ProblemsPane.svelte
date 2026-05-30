<script>
  let { diagnostics = [], onpick } = $props();
</script>

<div class="problems">
  {#if diagnostics.length === 0}
    <div class="empty">
      <span class="ok-dot"></span> No problems — the model is well-formed.
    </div>
  {:else}
    <ul>
      {#each diagnostics as d}
        <li class={d.severity}>
          <button
            type="button"
            class="row"
            onclick={() => onpick?.(d)}
            aria-label="{d.severity}{d.file ? ` in ${d.file}` : ''} at line {d.start_line} column {d.start_col}: {d.message}"
          >
            <span class="badge">{d.severity}</span>
            {#if d.file}<span class="file">{d.file}</span>{/if}
            <span class="loc">{d.start_line}:{d.start_col}</span>
            <span class="msg">{d.message}</span>
            {#if d.code}<span class="code">{d.code}</span>{/if}
          </button>
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
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 1rem 1.1rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--ink-soft);
  }
  .ok-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 18%, transparent);
  }
  ul { list-style: none; margin: 0; padding: 0; }
  li { border-bottom: 1px solid var(--line); }
  li.error { border-left: 2px solid var(--err); }
  li.warning { border-left: 2px solid var(--warn); }
  li.info { border-left: 2px solid var(--line-strong); }
  .row {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    padding: 0.45rem 1.1rem;
    color: inherit;
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }
  .row:hover { background: color-mix(in srgb, var(--accent) 6%, transparent); }
  li.error .row:hover { background: color-mix(in srgb, var(--err) 8%, transparent); }
  .badge {
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.58rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 0.12rem 0.4rem;
    border-radius: 4px;
  }
  li.error .badge { color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  li.warning .badge { color: var(--warn); background: color-mix(in srgb, var(--warn) 14%, transparent); }
  li.info .badge { color: var(--ink-soft); background: var(--surface-2); }
  .file { flex: none; font-family: var(--font-mono); font-size: 0.72rem; color: var(--accent); }
  .loc { flex: none; font-family: var(--font-mono); font-size: 0.72rem; color: var(--ink-faint); }
  .msg { flex: 1; min-width: 0; }
  .code { flex: none; font-family: var(--font-mono); font-size: 0.72rem; color: var(--ink-faint); }
</style>
