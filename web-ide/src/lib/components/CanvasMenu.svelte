<script lang="ts">
  // The canvas right-click menu, shared by the C4 graph and the sequence
  // timeline. It owns only the chrome — pointer-anchored box, dismissal, focus —
  // and renders a declarative list of sections; each caller builds the rows it
  // needs (drill / flows / definition / usages). One model, two diagrams.

  import type { MenuSection } from "$lib/core/types.js";

  type Props = {
    kind: string;
    label: string;
    x: number;
    y: number;
    sections: MenuSection[];
    onclose: () => void;
  };

  let { kind, label, x, y, sections, onclose }: Props = $props();

  let menuEl = $state<HTMLDivElement | null>(null);

  // Render under <body> so the menu's `position: fixed` resolves against the
  // viewport. Inside the canvas islands a transformed ancestor forms a containing
  // block, which would otherwise offset `left: x` by the rail + explorer width.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  // Run a row's action and dismiss.
  function act(run: () => void): void {
    run();
    onclose();
  }

  // Move keyboard focus to the menu when it opens so Escape works.
  $effect(() => {
    menuEl?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div use:portal>
  <!-- A transparent layer that dismisses on any click or another right-click. -->
  <button
    class="menu-scrim"
    aria-label="Close menu"
    onclick={onclose}
    oncontextmenu={(e) => {
      e.preventDefault();
      onclose();
    }}
  ></button>
  <div bind:this={menuEl} class="ctx-menu" role="menu" tabindex="-1" aria-label="Actions" style="left:{x}px; top:{y}px">
    <div class="ctx-head">
      <span class="kind {kind}">{kind}</span>
      <span class="ctx-name">{label}</span>
    </div>
    {#each sections as section, si (si)}
      <div class="ctx-sep"></div>
      {#if section.label}<div class="ctx-label">{section.label}</div>{/if}
      {#each section.items as item, ii (ii)}
        <button role="menuitem" class="ctx-item" onclick={() => act(item.run)}>
          {#if item.icon}<span class="play">{item.icon}</span>{/if}
          <span class="flow-name">{item.label}</span>
          {#if item.badge}<span class="trig">{item.badge}</span>{/if}
        </button>
      {/each}
    {/each}
  </div>
</div>

<style>
  .menu-scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }
  .ctx-menu {
    position: fixed;
    z-index: 61;
    min-width: 13rem;
    max-width: 18rem;
    padding: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    outline: none;
  }
  .ctx-head {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.3rem 0.45rem 0.4rem;
  }
  .ctx-head .kind {
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    border-left: 2px solid var(--k, var(--ink-faint));
    background: var(--surface-3);
  }
  .ctx-head .kind.person { --k: var(--k-person); }
  .ctx-head .kind.system { --k: var(--k-system); }
  .ctx-head .kind.container { --k: var(--k-container); }
  .ctx-head .kind.component { --k: var(--k-component); }
  .ctx-head .kind.data { --k: var(--k-data); }
  .ctx-head .kind.callable { --k: var(--k-callable); }
  .ctx-name {
    font-family: var(--font-display);
    font-size: 0.86rem;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx-sep {
    height: 1px;
    margin: 0.2rem 0.2rem;
    background: var(--line);
  }
  .ctx-label {
    padding: 0.3rem 0.5rem 0.15rem;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .ctx-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.4rem 0.5rem;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    color: var(--ink-soft);
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
  }
  .ctx-item:hover,
  .ctx-item:focus-visible {
    background: var(--surface-2);
    color: var(--ink);
    outline: none;
  }
  .ctx-item .flow-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx-item .play {
    flex: none;
    color: var(--k-callable);
    font-size: 0.58rem;
  }
  .ctx-item .trig {
    margin-left: auto;
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }
</style>
