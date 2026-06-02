<script lang="ts">
  import { Sparkles, Copy, Check } from '@lucide/svelte';

  // The literal prompt to paste into any shell-capable coding agent.
  const PROMPT =
    'Model this project using PseudoScript. Run `pds -h` to get started. '
    + 'Run `pds skill` and `pds lang` to learn the method and grammar, then write the model.';

  let copied = $state(false);
  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(PROMPT);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      copied = false;
    }
  }
</script>

<section id="agents" data-screen-label="With an agent">
  <div class="wrap">
    <div class="onramp-head">
      <div class="eyebrow-row reveal"><span class="lbl accent">With an agent</span></div>
      <h2 class="statement reveal d1">Don't want to write code? <span class="hot">Let the agent handle it.</span></h2>
      <p class="lede reveal d2">
        Point your agent at your codebase and the PseudoScript model writes itself. You review the diagrams, not the grammar. Or talk it through and let the agent scope it from the transcript.
      </p>
    </div>

    <div class="handover reveal d1">
      <div class="ho-head">
        <span class="ho-who"><span class="ho-ico"><Sparkles size={13} strokeWidth={2} /></span> Hand it to your agent</span>
        <button class="ho-copy" onclick={copy} aria-label="Copy the handover prompt">
          {#if copied}<Check size={14} strokeWidth={2} /> Copied{:else}<Copy size={14} strokeWidth={1.75} /> Copy prompt{/if}
        </button>
      </div>
      <div class="ho-body">
        <p>Model this project using PseudoScript. Run <code>pds -h</code> to get started.</p>
        <p>Run <code>pds skill</code> and <code>pds lang</code> to learn the method and grammar, then write the model.</p>
      </div>
      <div class="ho-foot"><span class="cursor"></span> paste into any coding agent with a shell</div>
    </div>
  </div>
</section>

<style>
  .onramp-head { max-width: 640px; margin: 0 0 2.4rem; }

  /* the handover prompt you give your agent — the section's one artefact */
  .handover { max-width: 620px; border: 1px solid var(--line-strong); border-radius: var(--radius); background: var(--surface); box-shadow: var(--shadow-lg); overflow: hidden; }
  .ho-head { display: flex; align-items: center; gap: .6rem; padding: .7rem .9rem; border-bottom: 1px solid var(--line); background: color-mix(in srgb, var(--surface-2) 80%, transparent); }
  .ho-who { display: inline-flex; align-items: center; gap: .45rem; font-family: var(--font-mono); font-size: .72rem; color: var(--ink-soft); }
  .ho-ico { display: inline-grid; place-items: center; color: var(--accent); }
  .ho-copy {
    margin-left: auto; display: inline-flex; align-items: center; gap: .4rem;
    padding: .35rem .65rem; border: 1px solid var(--line-strong); border-radius: var(--radius-sm);
    background: var(--surface-2); color: var(--ink-soft); font-family: var(--font-mono);
    font-size: .66rem; cursor: pointer; transition: color .14s, border-color .14s;
  }
  .ho-copy:hover { color: var(--ink); border-color: var(--accent); }

  .ho-body { padding: 1.2rem 1.2rem .4rem; }
  .ho-body p { margin: 0 0 1rem; font-size: .96rem; line-height: 1.65; color: var(--ink); }
  .ho-body code { font-family: var(--font-mono); font-size: .84em; color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, transparent); padding: .08em .35em; border-radius: 4px; }

  .ho-foot { display: flex; align-items: center; gap: .5rem; padding: .8rem 1.2rem 1.1rem; font-family: var(--font-mono); font-size: .68rem; color: var(--ink-faint); }
  .ho-foot .cursor { width: 7px; height: 1.05em; background: var(--accent); display: inline-block; animation: ho-blink 1.1s steps(1) infinite; }
  @keyframes ho-blink { 50% { opacity: 0; } }
  :global(body.no-motion) .ho-foot .cursor { animation: none; }
</style>
