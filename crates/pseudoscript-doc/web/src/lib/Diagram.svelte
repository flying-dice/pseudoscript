<script>
  import { onMount } from "svelte";

  let { diagram } = $props();
  let host = $state(null);

  // The diagram islands (Svelte Flow / timeline) are DOM-only: they are
  // imported dynamically and mounted on the client, so they never enter the
  // QuickJS SSR bundle. SSR emits the figure shell with an empty canvas.
  // An `svg` figure is pre-rendered upstream and needs no island.
  onMount(async () => {
    if (!host || diagram.diagram === "empty" || diagram.diagram === "svg") return;
    const { mount } = await import("svelte");
    const isC4 = diagram.diagram === "c4";
    const mod = isC4 ? await import("./C4Flow.svelte") : await import("./FlowTimeline.svelte");
    // The sequence timeline renders the positioned layout from the compiler (the
    // same geometry the web IDE draws); C4 lays itself out client-side.
    const props = isC4
      ? { scene: diagram.scene }
      : { scene: diagram.scene, layout: diagram.layout };
    mount(mod.default, { target: host, props });
  });
</script>

{#if diagram.diagram === "empty"}
  <div class="no-diagram">No {diagram.eyebrow} diagram available.</div>
{:else if diagram.diagram === "svg"}
  <figure class="figure">
    <figcaption>
      <span class="cap-title">{diagram.caption}</span>
    </figcaption>
    <div class="diagram diagram-svg" data-diagram={diagram.diagram}>
      <!-- eslint-disable-next-line svelte/no-at-html-tags -- compiler-rendered SVG, no user markup -->
      {@html diagram.svg}
    </div>
  </figure>
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
