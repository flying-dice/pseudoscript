<script lang="ts">
  // The system, shown live. A node scene renders an interactive C4 graph; a
  // sequence scene renders the animated timeline. The scene comes from the
  // compiler (emit_scene / symbol_scene), so the diagram type is read off the
  // scene itself — the pane is the model, not a picture of it.
  import C4Flow from "./C4Flow.svelte";
  import DataModel from "./DataModel.svelte";
  import FeatureFlow from "./FeatureFlow.svelte";
  import FlowTimeline from "./FlowTimeline.svelte";
  import { DEPTHS } from "$lib/sequence.js";
  import type { Depth } from "$lib/sequence.js";
  import type { ComponentProps } from "svelte";

  // The pane reads only the discriminating arrays off the scene (`participants`
  // for a sequence, `nodes` for C4, `entities` for a data ER view, `steps` for a
  // feature flow) and passes the whole object through to whichever child renders
  // it. The index signature keeps it a structural superset of every scene type.
  type Scene = {
    participants?: unknown[];
    nodes?: unknown[];
    entities?: unknown[];
    steps?: unknown[];
    [key: string]: unknown;
  };
  // The positioned sequence layout (opaque here — produced by the layout crate
  // and consumed only by FlowTimeline); the pane just forwards and signs it.
  type Layout = unknown;
  // An entry-point flow offered in a C4 node's popover.
  type Flow = { fqn: string; name: string; triggered?: boolean };

  type Props = {
    scene?: Scene | null;
    layout?: Layout | null;
    error?: string;
    hint?: string;
    onpick?: ((fqn: string) => void) | null;
    onup?: (() => void) | null;
    flows?: Map<string, Flow[]> | null;
    depth?: Depth;
    ondepth?: ((id: Depth) => void) | null;
    onusages?: ((fqn: string, event: MouseEvent) => void) | null;
    onsource?: ((fqn: string) => void) | null;
    typeFqn?: string | null;
  };

  let {
    scene = null,
    layout = null,
    error = "",
    hint = "Nothing to draw.",
    onpick,
    onup,
    flows = null,
    depth = "component",
    ondepth = null,
    onusages = null,
    onsource = null,
    typeFqn = null,
  }: Props = $props();

  const isFlow = $derived(!!scene && Array.isArray(scene.participants));
  const isData = $derived(!!scene && Array.isArray(scene.entities));
  const isFeature = $derived(!!scene && Array.isArray(scene.steps));
  const hasC4 = $derived(!!scene && Array.isArray(scene.nodes) && scene.nodes.length > 0);
  const hasFlow = $derived(isFlow && (scene?.participants?.length ?? 0) > 0);
  const hasData = $derived(isData && (scene?.entities?.length ?? 0) > 0);
  const hasFeature = $derived(isFeature && (scene?.steps?.length ?? 0) > 0);
  const ready = $derived(hasFlow || hasData || hasFeature || hasC4);
  // Remount the flow when the rendered content changes so the view resets. Both
  // kinds are now positioned by the layout engine, so key off the layout.
  const sig = $derived(layout ? JSON.stringify(layout) : scene ? JSON.stringify(scene) : "");
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
      {#if isFlow}<FlowTimeline scene={scene as ComponentProps<typeof FlowTimeline>["scene"]} layout={layout as ComponentProps<typeof FlowTimeline>["layout"]} {onusages} {onsource} {typeFqn} />{:else if isData}<DataModel scene={scene as ComponentProps<typeof DataModel>["scene"]} layout={layout as ComponentProps<typeof DataModel>["layout"]} {onpick} />{:else if isFeature}<FeatureFlow scene={scene as ComponentProps<typeof FeatureFlow>["scene"]} layout={layout as ComponentProps<typeof FeatureFlow>["layout"]} />{:else}<C4Flow scene={scene as ComponentProps<typeof C4Flow>["scene"]} layout={layout as ComponentProps<typeof C4Flow>["layout"]} {onpick} {onup} {flows} {onsource} {onusages} />{/if}
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
    background: var(--island-bg);
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
