<script>
  // A pre-rendered SVG figure: caption, pan/zoom toolbar, and the inline SVG
  // document. No event handlers here — behaviors.js wires the toolbar, wheel
  // zoom, and pointer pan by DOM, so the component never hydrates.
  let { diagram } = $props();
</script>

{#if diagram.diagram === "empty"}
  <p class="figure-empty">{diagram.caption}: no {diagram.eyebrow}.</p>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -- focusable so + - 0 f keyboard zoom works -->
  <figure class="figure" data-diagram={diagram.kind} tabindex="0">
    <figcaption>
      <span class="cap-text">
        <span class="cap-eyebrow">{diagram.eyebrow}</span>
        <span class="cap-title">{diagram.caption}</span>
      </span>
      <span class="hint">scroll to zoom &middot; drag to pan</span>
      <span class="fig-toolbar" role="group" aria-label="Diagram controls">
        <button class="fig-btn" type="button" data-fig="zoom-in" aria-label="Zoom in">+</button>
        <button class="fig-btn" type="button" data-fig="zoom-out" aria-label="Zoom out">&minus;</button>
        <button class="fig-btn" type="button" data-fig="reset" aria-label="Reset view">&#10226;</button>
        <button class="fig-btn" type="button" data-fig="fullscreen" aria-label="Fullscreen">&#10530;</button>
      </span>
    </figcaption>
    <div class="fig-viewport">
      <div class="fig-canvas">
        <!-- eslint-disable-next-line svelte/no-at-html-tags -- compiler-rendered SVG, no user markup -->
        {@html diagram.svg}
      </div>
    </div>
  </figure>
{/if}
