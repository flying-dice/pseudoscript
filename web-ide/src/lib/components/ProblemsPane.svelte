<script lang="ts">
  import { Copy } from "@lucide/svelte";

  type Problem = {
    severity: string;
    message: string;
    start_line: number;
    start_col: number;
    file?: string;
    code?: string;
    code_description?: string;
  };

  type Props = {
    diagnostics?: Problem[];
    onpick?: (diagnostic: Problem) => void;
    // Impure edge (clipboard write + toast) stays in the parent — see onshare.
    oncopy?: (text: string, count: number) => void;
  };

  let { diagnostics = [], onpick, oncopy }: Props = $props();

  // One problem as a plain-text line, ready to paste into an LLM.
  function format(d: Problem): string {
    const loc = d.file ? `${d.file}:${d.start_line}:${d.start_col}` : `${d.start_line}:${d.start_col}`;
    return `${d.severity} ${loc} ${d.message}${d.code ? ` [${d.code}]` : ""}`;
  }
</script>

<div class="problems" data-testid="problems-pane">
  {#if diagnostics.length === 0}
    <div class="empty">
      <span class="ok-dot"></span> No problems — the model is well-formed.
    </div>
  {:else}
    <div class="bar">
      <span class="count">{diagnostics.length} problem{diagnostics.length === 1 ? "" : "s"}</span>
      <button
        type="button"
        class="copy-all"
        data-testid="problems-copy-all"
        onclick={() => oncopy?.(diagnostics.map(format).join("\n"), diagnostics.length)}
        title="Copy all problems to the clipboard"
      >
        <Copy size={12} strokeWidth={2} aria-hidden="true" /> Copy all
      </button>
    </div>
    <ul>
      {#each diagnostics as d, i}
        <li class={d.severity}>
          <div class="row">
            <button
              type="button"
              class="nav"
              data-testid="problem-{i}"
              onclick={() => onpick?.(d)}
              aria-label="{d.severity}{d.file ? ` in ${d.file}` : ''} at line {d.start_line} column {d.start_col}: {d.message}"
            >
              <span class="badge">{d.severity}</span>
              {#if d.file}<span class="file" data-testid="problem-{i}-file">{d.file}</span>{/if}
              <span class="loc">{d.start_line}:{d.start_col}</span>
              <span class="msg">{d.message}</span>
            </button>
            {#if d.code}
              {#if d.code_description}
                <a
                  class="code link"
                  href={d.code_description}
                  target="_blank"
                  rel="noreferrer"
                  data-testid="problem-{i}-code"
                  title="{d.code} — open the principle article"
                >{d.code}</a>
              {:else}
                <span class="code" data-testid="problem-{i}-code">{d.code}</span>
              {/if}
            {/if}
            <button
              type="button"
              class="copy-one"
              onclick={() => oncopy?.(format(d), 1)}
              title="Copy this problem to the clipboard"
              aria-label="Copy problem to clipboard"
            >
              <Copy size={13} strokeWidth={2} aria-hidden="true" />
            </button>
          </div>
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
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    padding: 0.4rem 1.1rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface-2);
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--ink-soft);
  }
  .copy-all {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.18rem 0.5rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--ink-soft);
    font: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }
  .copy-all:hover { color: var(--ink); background: color-mix(in srgb, var(--accent) 8%, transparent); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { border-bottom: 1px solid var(--line); }
  li.error { border-left: 2px solid var(--err); }
  li.warning { border-left: 2px solid var(--warn); }
  li.info { border-left: 2px solid var(--line-strong); }
  .row { display: flex; align-items: stretch; }
  .row:hover { background: color-mix(in srgb, var(--accent) 6%, transparent); }
  li.error .row:hover { background: color-mix(in srgb, var(--err) 8%, transparent); }
  .nav {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    flex: 1;
    min-width: 0;
    text-align: left;
    background: transparent;
    border: none;
    padding: 0.45rem 0.6rem 0.45rem 1.1rem;
    color: inherit;
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }
  .copy-one {
    flex: none;
    display: flex;
    align-items: center;
    padding: 0 0.7rem;
    background: transparent;
    border: none;
    color: var(--ink-faint);
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .copy-one,
  .copy-one:focus-visible { opacity: 1; }
  .copy-one:hover { color: var(--accent); }
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
  .code {
    flex: none;
    display: flex;
    align-items: center;
    padding: 0 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink-faint);
    text-decoration: none;
  }
  a.code.link { color: var(--accent); cursor: pointer; }
  a.code.link:hover,
  a.code.link:focus-visible { text-decoration: underline; }
</style>
