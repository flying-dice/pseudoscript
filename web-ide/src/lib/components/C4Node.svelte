<script lang="ts">
  // A peer card in the C4 graph: kind tag, title, and the `///` summary as a
  // dimmed description. Replaces Svelte Flow's default node (which renders only
  // the label) so the card carries the same context the info popover does.
  // Hidden handles let edges attach; the card itself stays non-interactive —
  // clicks bubble to the flow's node-click handler that opens the popover.
  import { Handle, Position } from "@xyflow/svelte";

  type C4Data = {
    label: string;
    kind: string;
    summary?: string;
  };

  type Props = {
    data: C4Data;
  };

  let { data }: Props = $props();
</script>

<Handle type="target" position={Position.Top} class="c4-handle" />
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
</style>
