<script>
  // The enclosing box of a drilled-in container/component view. Renders the
  // boundary's title and a close button that pops up one level to the parent
  // scope. Hidden handles let cross-boundary edges still attach.
  import { Handle, Position } from "@xyflow/svelte";

  let { data } = $props();
</script>

<Handle type="target" position={Position.Top} class="boundary-handle" />
<div class="boundary-head">
  <span class="boundary-title">{data.label}</span>
  <button
    class="boundary-close"
    title="Close — go up a level"
    aria-label="Close and go up a level"
    onpointerdown={(e) => e.stopPropagation()}
    onclick={(e) => {
      e.stopPropagation();
      data.onclose?.();
    }}
  >✕</button>
</div>
<Handle type="source" position={Position.Bottom} class="boundary-handle" />

<style>
  .boundary-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .boundary-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .boundary-close {
    flex: none;
    width: 1.3rem;
    height: 1.3rem;
    display: grid;
    place-items: center;
    border-radius: var(--radius-sm);
    border: 1px solid var(--k, var(--line-strong));
    background: var(--surface-2);
    color: var(--ink-soft);
    font-size: 0.7rem;
    line-height: 1;
    cursor: pointer;
    transition: background 0.13s, color 0.13s, border-color 0.13s;
  }
  .boundary-close:hover {
    background: var(--k, var(--accent));
    color: var(--accent-ink);
    border-color: var(--k, var(--accent));
  }
  :global(.boundary-handle) {
    opacity: 0;
    pointer-events: none;
  }
</style>
