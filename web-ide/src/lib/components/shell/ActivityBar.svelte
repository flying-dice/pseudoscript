<script lang="ts">
  import type { Component as ComponentType } from "svelte";

  import { Files, Network, PanelRight, Settings } from "@lucide/svelte";

  import * as Tooltip from "$lib/components/ui/tooltip/index.js";

  // The two work modes the bar switches the centre between.
  type Activity = "explorer" | "canvas";

  type Props = {
    active?: Activity;
    structureOpen?: boolean;
    onselect?: (activity: Activity) => void;
    ontogglestructure?: () => void;
    onsettings?: () => void;
  };

  let { active = "explorer", structureOpen = true, onselect, ontogglestructure, onsettings }: Props = $props();
</script>

<Tooltip.Provider delayDuration={250}>
  <nav class="activity island" aria-label="Activity bar">
    <div class="grp">
      {@render item(Files, "Explorer", () => onselect?.("explorer"), active === "explorer")}
      {@render item(Network, "Canvas", () => onselect?.("canvas"), active === "canvas")}
    </div>
    <div class="grp bottom">
      {@render item(PanelRight, structureOpen ? "Hide structure" : "Show structure", () => ontogglestructure?.(), structureOpen)}
      {@render item(Settings, "Settings", () => onsettings?.(), false)}
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
  .activity {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: center;
    padding: 0.4rem 0;
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    border-right: 1px solid var(--line);
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
</style>
