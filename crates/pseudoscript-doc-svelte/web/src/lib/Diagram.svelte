<script>
  import { onMount } from "svelte";

  let { diagram } = $props();
  let host = $state(null);

  // The diagram islands (Svelte Flow / timeline) are DOM-only: they are
  // imported dynamically and mounted on the client, so they never enter the
  // QuickJS SSR bundle. SSR emits the figure shell with an empty canvas.
  onMount(async () => {
    if (!host || diagram.diagram === "empty") return;
    const { mount } = await import("svelte");
    const mod =
      diagram.diagram === "c4"
        ? await import("./C4Flow.svelte")
        : await import("./FlowTimeline.svelte");
    mount(mod.default, { target: host, props: { scene: diagram.scene } });
  });
</script>

{#if diagram.diagram === "empty"}
  <div class="no-diagram">No {diagram.eyebrow} diagram available.</div>
{:else}
  <figure class="figure">
    <figcaption>
      <span class="cap-title">{diagram.caption}</span>
      <span class="hint">
        {diagram.diagram === "sequence"
          ? "play · scrub the flow"
          : "scroll to zoom · drag to pan"}
      </span>
    </figcaption>
    <div class="diagram" data-diagram={diagram.diagram} bind:this={host}></div>
  </figure>
{/if}
