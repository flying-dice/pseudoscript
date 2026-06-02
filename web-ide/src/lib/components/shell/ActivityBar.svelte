<script lang="ts">
  import type { Component as ComponentType } from "svelte";

  import { Files, Network, TriangleAlert } from "@lucide/svelte";

  import * as Tooltip from "$lib/components/ui/tooltip/index.js";

  // The two work modes the bar switches the centre between.
  type Activity = "explorer" | "canvas";

  type Props = {
    active?: Activity;
    explorerOpen?: boolean;
    problemsOpen?: boolean;
    problemCount?: number;
    errorCount?: number;
    onexplorer?: () => void;
    oncanvas?: () => void;
    ontoggleproblems?: () => void;
  };

  let {
    active = "explorer",
    explorerOpen = true,
    problemsOpen = false,
    problemCount = 0,
    errorCount = 0,
    onexplorer,
    oncanvas,
    ontoggleproblems,
  }: Props = $props();

  // The problems severity: green when clean, amber for warnings, red for errors.
  const severity = $derived(errorCount > 0 ? "err" : problemCount > 0 ? "warn" : "ok");
</script>

<Tooltip.Provider delayDuration={250}>
  <nav class="activity" aria-label="Activity bar">
    <div class="grp">
      {@render item(Files, active === "explorer" && explorerOpen ? "Hide explorer" : "Explorer", () => onexplorer?.(), active === "explorer" && explorerOpen)}
      {@render item(Network, "Canvas", () => oncanvas?.(), active === "canvas")}
    </div>
    <div class="grp bottom">
      <Tooltip.Root>
        <Tooltip.Trigger
          class="act-btn problems sev-{severity} {problemsOpen ? 'on' : ''}"
          onclick={() => ontoggleproblems?.()}
          aria-label={problemCount === 0 ? "No problems" : `${problemCount} problem${problemCount === 1 ? "" : "s"}`}
          aria-pressed={problemsOpen}
        >
          <TriangleAlert size={18} strokeWidth={1.75} aria-hidden="true" />
          {#if problemCount > 0}<span class="count" aria-hidden="true">{problemCount}</span>{/if}
        </Tooltip.Trigger>
        <Tooltip.Content side="right" sideOffset={8}>
          {problemCount === 0 ? "No problems" : `${problemCount} problem${problemCount === 1 ? "" : "s"}`}
        </Tooltip.Content>
      </Tooltip.Root>
    </div>
  </nav>
</Tooltip.Provider>

{#snippet item(Icon: ComponentType, label: string, onclick: () => void, on: boolean)}
  <Tooltip.Root>
    <Tooltip.Trigger class="act-btn {on ? 'on' : ''}" {onclick} aria-label={label} aria-pressed={on}>
      <Icon size={18} strokeWidth={1.75} aria-hidden="true" />
    </Tooltip.Trigger>
    <Tooltip.Content side="right" sideOffset={8}>{label}</Tooltip.Content>
  </Tooltip.Root>
{/snippet}

<style>
  /* A bare rail — no island chrome (no background, no border); the icons float
     over the body backdrop, JetBrains tool-window-stripe style. */
  .activity {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: center;
    padding: 0.4rem 0;
    background: none;
    border: none;
  }
  .grp {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .activity :global(.act-btn) {
    position: relative;
    width: 2rem;
    height: 2rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    cursor: pointer;
    transition:
      color 0.12s,
      background 0.12s;
  }
  .activity :global(.act-btn:hover) {
    color: var(--ink);
    background: var(--surface-2);
  }
  .activity :global(.act-btn.on) {
    color: var(--accent);
  }
  /* active rail */
  .activity :global(.act-btn.on)::before {
    content: "";
    position: absolute;
    left: -0.4rem;
    top: 0.3rem;
    bottom: 0.3rem;
    width: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  /* Problems icon carries its own severity colour (green → amber → red), which
     wins over the hover/active tints — these rules come last. */
  .activity :global(.act-btn.sev-ok) {
    color: var(--ok);
  }
  .activity :global(.act-btn.sev-warn) {
    color: var(--warn);
  }
  .activity :global(.act-btn.sev-err) {
    color: var(--err);
  }
  /* count bubble, bottom-right of the icon, filled to match the severity (it only
     shows when problemCount > 0, so the fill is always amber or red) */
  .count {
    position: absolute;
    bottom: -0.05rem;
    right: -0.1rem;
    min-width: 0.85rem;
    height: 0.85rem;
    padding: 0 0.18rem;
    display: grid;
    place-items: center;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    line-height: 1;
    border-radius: 999px;
    color: var(--bg);
  }
  :global(.act-btn.sev-warn) .count {
    background: var(--warn);
  }
  :global(.act-btn.sev-err) .count {
    background: var(--err);
  }
</style>
