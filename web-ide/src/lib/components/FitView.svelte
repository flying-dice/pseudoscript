<script lang="ts">
  // Re-frames the C4 canvas whenever `sig` changes — i.e. when a new layout
  // algorithm or direction moves the nodes. Lives inside <SvelteFlow> so it can
  // reach the flow instance; runs after a couple of frames so the moved nodes
  // are laid out before fitView measures their bounds.
  import { useSvelteFlow } from "@xyflow/svelte";

  let { sig }: { sig: string } = $props();

  const { fitView } = useSvelteFlow();

  $effect(() => {
    sig; // re-fit on every change of the layout signature
    let cancelled = false;
    const id = requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (!cancelled) fitView({ padding: 0.18 });
      }),
    );
    return () => {
      cancelled = true;
      cancelAnimationFrame(id);
    };
  });
</script>
