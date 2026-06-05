<script lang="ts">
  // The system, shown live. A node scene renders an interactive C4 graph; a
  // sequence scene renders the animated timeline. The scene comes from the
  // compiler (emit_scene / symbol_scene), so the diagram type is read off the
  // scene itself — the pane is the model, not a picture of it.
  import C4Flow from "./C4Flow.svelte";
  import DataModel from "./DataModel.svelte";
  import FeatureFlow from "./FeatureFlow.svelte";
  import FlowTimeline from "./FlowTimeline.svelte";
  import type { Depth } from "$lib/sequence.js";
  import type { LayoutTweaks } from "$lib/core/types.js";
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
    tweaks?: LayoutTweaks | null;
    onlayoutchange?: ((tweaks: LayoutTweaks) => void) | null;
    unlocked?: boolean;
    onpin?: ((fqn: string, row: number, col: number) => void) | null;
    onunlock?: ((next: boolean) => void) | null;
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
    tweaks = null,
    onlayoutchange = null,
    unlocked = false,
    onpin = null,
    onunlock = null,
  }: Props = $props();

  const isFlow = $derived(!!scene && Array.isArray(scene.participants));
  const isData = $derived(!!scene && Array.isArray(scene.entities));
  const isFeature = $derived(!!scene && Array.isArray(scene.steps));
  const hasC4 = $derived(!!scene && Array.isArray(scene.nodes) && scene.nodes.length > 0);
  const hasFlow = $derived(isFlow && (scene?.participants?.length ?? 0) > 0);
  const hasData = $derived(isData && (scene?.entities?.length ?? 0) > 0);
  const hasFeature = $derived(isFeature && (scene?.steps?.length ?? 0) > 0);
  const ready = $derived(hasFlow || hasData || hasFeature || hasC4);
  // Remount the flow when the *diagram* changes so the view resets — but not on a
  // mere re-layout of the same C4 view (a pin drag or a dial change repositions
  // nodes reactively, keeping zoom/pan). So for C4, key on the view subject + node
  // set, not the geometry; other diagram kinds keep the geometry key.
  const c4NodeKey = $derived.by(() => {
    if (!scene || !Array.isArray(scene.nodes)) return null;
    const ln = (layout as { nodes?: { fqn?: string }[] } | null)?.nodes;
    if (!Array.isArray(ln)) return null;
    return ln.map((n) => n.fqn ?? "").join(",");
  });
  const sig = $derived(
    c4NodeKey !== null
      ? `c4|${(scene?.of as string | undefined) ?? ""}|${c4NodeKey}`
      : layout
        ? JSON.stringify(layout)
        : scene
          ? JSON.stringify(scene)
          : "",
  );
</script>

<div class="stage" class:framed={ready}>
  {#if error}
    <div class="note error">
      <span class="kicker">cannot project</span>
      <p>{error}</p>
    </div>
  {:else if ready}
    {#key sig}
      {#if isFlow}<FlowTimeline scene={scene as ComponentProps<typeof FlowTimeline>["scene"]} layout={layout as ComponentProps<typeof FlowTimeline>["layout"]} {onusages} {onsource} {typeFqn} {depth} {ondepth} />{:else if isData}<DataModel scene={scene as ComponentProps<typeof DataModel>["scene"]} layout={layout as ComponentProps<typeof DataModel>["layout"]} {onpick} />{:else if isFeature}<FeatureFlow scene={scene as ComponentProps<typeof FeatureFlow>["scene"]} layout={layout as ComponentProps<typeof FeatureFlow>["layout"]} />{:else}<C4Flow scene={scene as ComponentProps<typeof C4Flow>["scene"]} layout={layout as ComponentProps<typeof C4Flow>["layout"]} {onpick} {onup} {flows} {onsource} {onusages} {tweaks} {onlayoutchange} {unlocked} {onpin} {onunlock} />{/if}
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
