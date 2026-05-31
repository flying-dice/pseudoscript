<script>
  // The system, shown live. A node scene renders an interactive C4 graph; a
  // sequence scene renders the animated timeline. The scene comes from the
  // compiler (emit_scene / symbol_scene), so the diagram type is read off the
  // scene itself — the pane is the model, not a picture of it.
  import C4Flow from "./C4Flow.svelte";
  import FlowTimeline from "./FlowTimeline.svelte";
  import { DEPTHS } from "$lib/sequence.js";

  let { scene = null, layout = null, error = "", hint = "Nothing to draw.", onpick, onup, flows = null, depth = "component", ondepth = null, oninfo = null, oninfoend = null, onusages = null, typeFqn = null } = $props();

  const isFlow = $derived(!!scene && Array.isArray(scene.participants));
  const hasC4 = $derived(!!scene && Array.isArray(scene.nodes) && scene.nodes.length > 0);
  const hasFlow = $derived(isFlow && scene.participants.length > 0);
  const ready = $derived(isFlow ? hasFlow : hasC4);
  // Remount the flow when the rendered content changes so the view resets.
  const sig = $derived(isFlow ? JSON.stringify(layout) : scene ? JSON.stringify(scene) : "");
</script>

<div class="stage" class:framed={ready}>
  {#if error}
    <div class="note error">
      <span class="kicker">cannot project</span>
      <p>{error}</p>
    </div>
  {:else if ready}
    {#if isFlow && ondepth}
      <div class="depth" role="group" aria-label="Sequence depth">
        {#each DEPTHS as d (d.id)}
          <button class:active={depth === d.id} onclick={() => ondepth(d.id)}>{d.label}</button>
        {/each}
      </div>
    {/if}
    {#key sig}
      {#if isFlow}<FlowTimeline {scene} {layout} {oninfo} {oninfoend} {onusages} {typeFqn} />{:else}<C4Flow {scene} {onpick} {onup} {flows} />{/if}
    {/key}
  {:else}
    <div class="note">
      <span class="kicker">diagram</span>
      <p>{hint}</p>
    </div>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    height: 100%;
    min-height: 0;
    background:
      radial-gradient(900px 520px at 60% -10%, color-mix(in srgb, var(--accent) 6%, transparent), transparent 70%),
      var(--bg);
  }
  /* centre the placeholder notes; the flow components fill the stage themselves */
  .stage:not(.framed) {
    display: grid;
    place-items: center;
    padding: 1.6rem;
  }
  /* depth selector: a segmented control floating over the sequence canvas */
  .depth {
    position: absolute;
    top: 0.7rem;
    right: 0.7rem;
    z-index: 5;
    display: flex;
    gap: 1px;
    padding: 2px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
  }
  .depth button {
    padding: 0.28rem 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--ink-soft);
    background: transparent;
    border: 0;
    border-radius: calc(var(--radius-sm) - 2px);
    cursor: pointer;
    white-space: nowrap;
  }
  .depth button:hover { color: var(--ink); }
  .depth button.active { background: var(--accent); color: var(--accent-ink); }

  .note { max-width: 30rem; text-align: center; color: var(--ink-soft); }
  .note .kicker {
    display: inline-block;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    margin-bottom: 0.6rem;
  }
  .note.error .kicker { color: var(--err); }
  .note p { margin: 0; font-family: var(--font-mono); font-size: 0.82rem; line-height: 1.7; }
  .note.error p { color: var(--err); }
</style>
