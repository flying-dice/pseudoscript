<script lang="ts">
  // Canvas customisation: a button floating over the C4 graph opens a modal that
  // controls the layout algorithm, its flow direction, and the edge line style.
  // Every choice is persisted by the canvas-prefs store and applied live. The
  // direction control disables itself for algorithms that don't use it.
  import { SlidersHorizontal } from "@lucide/svelte";

  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import {
    ALGORITHMS,
    EDGE_STYLES,
    LAYOUTS,
    canvasPrefs,
    isDirectional,
  } from "$lib/stores/canvas-prefs.svelte.js";
  import type { EdgeStyle, LayoutAlgo, LayoutDir } from "$lib/stores/canvas-prefs.svelte.js";

  const directional = $derived(isDirectional(canvasPrefs.algorithm));
</script>

<!-- A labelled select row; `disabled` greys it out (used for direction when the
     algorithm doesn't honour it). Defined at template scope so it isn't mistaken
     for a Dialog.Content prop. -->
{#snippet row(id: string, label: string, value: string, options: readonly { id: string; label: string }[], onpick: (v: string) => void, disabled = false)}
  <div class="row" class:disabled>
    <label for={id}>{label}</label>
    <select {id} {value} {disabled} onchange={(e) => onpick(e.currentTarget.value)}>
      {#each options as o (o.id)}
        <option value={o.id}>{o.label}</option>
      {/each}
    </select>
  </div>
{/snippet}

<Dialog.Root>
  <Dialog.Trigger class="customise-trigger" aria-label="Customise diagram">
    <SlidersHorizontal size={13} strokeWidth={2} aria-hidden="true" />
    Customise
  </Dialog.Trigger>
  <Dialog.Content class="sm:max-w-sm" data-testid="canvas-settings">
    <Dialog.Header>
      <Dialog.Title>Customise diagram</Dialog.Title>
      <Dialog.Description>Layout and edges. Saved across sessions.</Dialog.Description>
    </Dialog.Header>

    <div class="rows">
      {@render row("cs-algo", "Algorithm", canvasPrefs.algorithm, ALGORITHMS, (v) => canvasPrefs.setAlgorithm(v as LayoutAlgo))}
      {@render row("cs-dir", "Direction", canvasPrefs.layout, LAYOUTS, (v) => canvasPrefs.setLayout(v as LayoutDir), !directional)}
      {@render row("cs-edge", "Lines", canvasPrefs.edge, EDGE_STYLES, (v) => canvasPrefs.setEdge(v as EdgeStyle))}
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.customise-trigger) {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.28rem 0.55rem;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--ink-soft);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    cursor: pointer;
    white-space: nowrap;
  }
  :global(.customise-trigger:hover),
  :global(.customise-trigger[data-state="open"]) {
    color: var(--ink);
    border-color: var(--accent);
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  .row {
    display: grid;
    grid-template-columns: 6rem 1fr;
    align-items: center;
    gap: 0.6rem;
  }
  .row.disabled label {
    color: var(--ink-faint);
  }
  .row label {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .row select {
    appearance: none;
    width: 100%;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.74rem;
    padding: 0.3rem 1.6rem 0.3rem 0.5rem;
    cursor: pointer;
    background-image: linear-gradient(45deg, transparent 50%, var(--ink-faint) 50%),
      linear-gradient(135deg, var(--ink-faint) 50%, transparent 50%);
    background-position:
      right 0.7rem center,
      right 0.45rem center;
    background-size:
      0.3rem 0.3rem,
      0.3rem 0.3rem;
    background-repeat: no-repeat;
  }
  .row select:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .row select:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
