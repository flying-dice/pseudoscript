<script lang="ts">
  import { CircleCheck, TriangleAlert } from "@lucide/svelte";

  import * as Popover from "$lib/components/ui/popover/index.js";
  import ProblemsPane from "../ProblemsPane.svelte";

  type Problem = { severity: string; message: string; file?: string; start_line: number; start_col: number; code?: string };

  type Props = {
    problems?: Problem[];
    errorCount?: number;
    onpick?: (d: Problem) => void;
  };

  let { problems = [], errorCount = 0, onpick }: Props = $props();
  let open = $state(false);

  const warnCount = $derived(problems.length - errorCount);
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    class="problems-badge {errorCount ? 'has-errors' : warnCount ? 'has-warns' : 'clean'}"
    aria-label="{problems.length} problem{problems.length === 1 ? '' : 's'}"
    title="Problems"
  >
    {#if problems.length}
      <TriangleAlert size={13} strokeWidth={2} aria-hidden="true" />
      <span class="count">{problems.length}</span>
    {:else}
      <CircleCheck size={13} strokeWidth={2} aria-hidden="true" />
    {/if}
  </Popover.Trigger>
  <Popover.Content class="problems-pop" align="end" sideOffset={8}>
    <div class="problems-pop-body">
      <ProblemsPane
        diagnostics={problems}
        onpick={(d) => {
          onpick?.(d);
          open = false;
        }}
      />
    </div>
  </Popover.Content>
</Popover.Root>

<style>
  :global(.problems-badge) {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.55rem;
    padding: 0 0.55rem;
    border-radius: 999px;
    border: 1px solid var(--line-strong);
    background: var(--surface-2);
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    cursor: pointer;
    transition:
      color 0.12s,
      border-color 0.12s;
  }
  :global(.problems-badge:hover) {
    color: var(--ink);
  }
  :global(.problems-badge.clean) {
    color: var(--ok);
    border-color: color-mix(in srgb, var(--ok) 35%, var(--line-strong));
  }
  :global(.problems-badge.has-warns) {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, var(--line-strong));
  }
  :global(.problems-badge.has-errors) {
    color: var(--err);
    border-color: color-mix(in srgb, var(--err) 50%, var(--line-strong));
  }
  :global(.problems-pop) {
    width: min(34rem, 92vw);
    max-height: min(60vh, 30rem);
    overflow: auto;
    padding: 0;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
  }
  .problems-pop-body {
    min-height: 0;
  }
</style>
