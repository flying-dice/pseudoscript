<script>
  // A sequence participant, drawn from the engine's placed coordinates: the
  // kind-coloured C4 head card, the dashed lifeline, and the execution-activation
  // bar spanning its involvement. The node fills the canvas, so every coordinate
  // here is absolute (no geometry is computed in the component).
  let { data } = $props();

  const p = data.placed;
  const act = data.act;

  // Canvas interaction mirrors the editor: hover shows info, Cmd/Ctrl-click shows
  // usages. Every lifeline participates — declared nodes resolve to their doc and
  // usages; synthesised trigger actors (client/scheduler/event) get a blurb.
  const interactive = !!p.id;
  const onclick = (e) => {
    if (interactive && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      data.onusages?.(p.id, e);
    }
  };
</script>

<div class="seq-life">
  <div
    class="seq-card c4-node {p.kind}"
    class:interactive
    style="left:{p.card.x}px; top:{p.card.y}px; width:{p.card.w}px; height:{p.card.h}px"
    role={interactive ? "button" : undefined}
    tabindex={interactive ? 0 : undefined}
    onmouseenter={(e) => interactive && data.oninfo?.(p.id, e)}
    onmouseleave={() => interactive && data.oninfoend?.()}
    {onclick}
  >
    <span class="seq-kind">{p.kind}</span>
    <span class="seq-name">{p.label}</span>
  </div>

  <svg class="seq-overlay" width="100%" height="100%">
    <!-- dashed lifeline -->
    <line x1={p.lifeline_x} y1={p.top} x2={p.lifeline_x} y2={p.bottom} class="seq-lifeline" />

    <!-- execution-activation bar -->
    {#if act}
      <rect
        class="seq-act {act.owner ? 'seq-act-owner' : ''}"
        x={act.x - 5}
        y={act.top}
        width="10"
        height={Math.max(10, act.bottom - act.top)}
        rx="2"
      />
    {/if}
  </svg>
</div>

<style>
  .seq-life {
    position: relative;
    width: 100%;
    height: 100%;
    background: transparent;
    pointer-events: none;
  }
  /* the C4 head card: kind eyebrow over the name, a kind-coloured left stripe */
  .seq-card {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.15rem;
    padding: 0 0.7rem;
    overflow: hidden;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-left: 3px solid var(--k, var(--ink-faint));
    border-radius: var(--radius-sm);
    box-shadow: 0 8px 20px -12px rgba(0, 0, 0, 0.8);
  }
  .seq-card.interactive { pointer-events: auto; cursor: pointer; }
  .seq-card.interactive:hover { border-color: var(--accent); }
  .seq-card.person { --k: var(--k-person); }
  .seq-card.system { --k: var(--k-system); }
  .seq-card.container { --k: var(--k-container); }
  .seq-card.component { --k: var(--k-component); }
  .seq-card.data { --k: var(--k-data); }
  .seq-card.callable { --k: var(--k-callable); }
  .seq-kind {
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--k, var(--ink-faint));
  }
  .seq-name {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .seq-overlay {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .seq-lifeline {
    stroke: var(--line-strong);
    stroke-dasharray: 2 4;
  }
  .seq-act {
    fill: var(--surface-3);
    stroke: var(--line-strong);
  }
  .seq-act-owner {
    fill: color-mix(in srgb, var(--accent) 18%, transparent);
    stroke: color-mix(in srgb, var(--accent) 55%, transparent);
  }
</style>
