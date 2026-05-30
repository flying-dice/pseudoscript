<script>
  // Live diagram preview. The SVG comes straight from the wasm compiler's
  // `emit_svg`; an un-projectable view (e.g. a container view with no target)
  // shows a hint instead of an error.
  let { svg = "", error = "" } = $props();
</script>

<div class="diagram">
  {#if error}
    <div class="placeholder">{error}</div>
  {:else if svg}
    <!-- trusted: emit_svg output is compiler-generated, not user HTML -->
    <div class="svg">{@html svg}</div>
  {:else}
    <div class="placeholder">No diagram for this view.</div>
  {/if}
</div>

<style>
  .diagram {
    height: 100%;
    overflow: auto;
    background: #f4f4f6; /* light plate so dark-on-light SVGs read in dark mode */
    display: grid;
    place-items: center;
    padding: 1.2rem;
  }
  .svg :global(svg) {
    max-width: 100%;
    height: auto;
    display: block;
  }
  .placeholder {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: #6c6e77;
  }
</style>
