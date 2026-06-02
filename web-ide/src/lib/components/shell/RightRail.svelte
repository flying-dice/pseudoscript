<script lang="ts">
  import { PanelRight } from "@lucide/svelte";

  import * as Tooltip from "$lib/components/ui/tooltip/index.js";

  type Props = {
    structureOpen?: boolean;
    ontogglestructure?: () => void;
  };

  let { structureOpen = true, ontogglestructure }: Props = $props();
</script>

<Tooltip.Provider delayDuration={250}>
  <nav class="rail" aria-label="Right tool bar">
    <Tooltip.Root>
      <Tooltip.Trigger
        class="act-btn {structureOpen ? 'on' : ''}"
        onclick={() => ontogglestructure?.()}
        aria-label={structureOpen ? "Hide structure" : "Show structure"}
        aria-pressed={structureOpen}
      >
        <PanelRight size={18} strokeWidth={1.75} aria-hidden="true" />
      </Tooltip.Trigger>
      <Tooltip.Content side="left" sideOffset={8}>{structureOpen ? "Hide structure" : "Show structure"}</Tooltip.Content>
    </Tooltip.Root>
  </nav>
</Tooltip.Provider>

<style>
  /* Bare rail mirroring ActivityBar — no background, no border. The active rail
     accent sits on the right edge (the rail hugs the right side of the window). */
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0.4rem 0;
    background: none;
    border: none;
  }
  .rail :global(.act-btn) {
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
  .rail :global(.act-btn:hover) {
    color: var(--ink);
    background: var(--surface-2);
  }
  .rail :global(.act-btn.on) {
    color: var(--accent);
  }
  .rail :global(.act-btn.on)::after {
    content: "";
    position: absolute;
    right: -0.4rem;
    top: 0.3rem;
    bottom: 0.3rem;
    width: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
</style>
