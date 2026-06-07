<script lang="ts">
  // A peer card in the C4 graph: kind tag, title, and the `///` summary as a
  // dimmed description. Replaces Svelte Flow's default node (which renders only
  // the label) so the card carries the same context the info popover does.
  // Hidden handles let edges attach; the card itself stays non-interactive —
  // clicks bubble to the flow's node-click handler that opens the popover.
  import { Handle, Position } from "@xyflow/svelte";
  import { Pin, X } from "@lucide/svelte";

  type C4Data = {
    label: string;
    kind: string;
    summary?: string;
    // Grid mode: whether this card is pinned to a cell, whether the grid is
    // unlocked (so the inline clear shows), and the clear-this-pin callback.
    pinned?: boolean;
    unlocked?: boolean;
    onunpin?: () => void;
  };

  type Props = {
    data: C4Data;
  };

  let { data }: Props = $props();
</script>

<Handle type="target" position={Position.Top} class="c4-handle" />
{#if data.pinned}
  <span class="pin-corner">
    <span class="pin-badge" data-testid="pin-badge" title="Pinned to a grid cell" aria-label="Pinned to a grid cell">
      <Pin size={11} strokeWidth={2.25} aria-hidden="true" />
    </span>
    {#if data.unlocked}
      <button
        class="unpin-btn"
        data-testid="unpin-btn"
        title="Clear this node's position"
        aria-label="Clear this node's position"
        onpointerdown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          data.onunpin?.();
        }}
      >
        <X size={11} strokeWidth={2.5} aria-hidden="true" />
      </button>
    {/if}
  </span>
{/if}
<span class="c4-kind">{data.kind}</span>
<span class="c4-label">{data.label}</span>
{#if data.summary}<span class="c4-summary">{data.summary}</span>{/if}
<Handle type="source" position={Position.Bottom} class="c4-handle" />

<style>
  .c4-kind {
    display: block;
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--k, var(--ink-faint));
  }
  .c4-label {
    display: block;
    margin-top: 0.2rem;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--ink);
  }
  .c4-summary {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    margin-top: 0.3rem;
    font-family: var(--font-sans, inherit);
    font-size: 0.68rem;
    font-weight: 400;
    line-height: 1.45;
    color: var(--ink-soft);
  }
  :global(.c4-handle) {
    opacity: 0;
    pointer-events: none;
  }

  /* Pin marker + inline clear, top-right corner of a pinned card. */
  .pin-corner {
    position: absolute;
    top: -0.55rem;
    right: -0.4rem;
    display: flex;
    align-items: center;
    gap: 0.15rem;
    z-index: 2;
  }
  .pin-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.05rem;
    height: 1.05rem;
    border-radius: 999px;
    background: var(--k-callable, #2563eb);
    color: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }
  .unpin-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.05rem;
    height: 1.05rem;
    padding: 0;
    border: 1px solid var(--line-strong, #888);
    border-radius: 999px;
    background: var(--surface, #fff);
    color: var(--ink, #111);
    font-size: 0.62rem;
    line-height: 1;
    cursor: pointer;
  }
  .unpin-btn:hover {
    border-color: var(--k-callable, #2563eb);
    color: var(--k-callable, #2563eb);
  }
</style>
