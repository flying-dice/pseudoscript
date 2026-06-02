<script lang="ts">
  import { File, FileCode, FileText, Settings2, X } from "@lucide/svelte";
  import type { Component as ComponentType } from "svelte";

  type Tab = { key: string; label: string; kind: "module" | "doc" | "manifest" | "other"; active: boolean; dirty: boolean };

  type Props = {
    tabs?: Tab[];
    onselect?: (key: string) => void;
    onclose?: (key: string) => void;
  };

  let { tabs = [], onselect, onclose }: Props = $props();

  const ICON: Record<Tab["kind"], ComponentType> = { module: FileCode, doc: FileText, manifest: Settings2, other: File };
</script>

<div class="tabbar" role="tablist" aria-label="Open files">
  {#each tabs as tab (tab.key)}
    {@const Icon = ICON[tab.kind]}
    <div
      class="tab"
      class:active={tab.active}
      onauxclick={(e) => {
        if (e.button === 1) {
          e.preventDefault();
          onclose?.(tab.key);
        }
      }}
    >
      <button class="tab-open" role="tab" aria-selected={tab.active} onclick={() => onselect?.(tab.key)} title={tab.key}>
        <Icon class="tab-ico" size={13} strokeWidth={2} aria-hidden="true" />
        <span class="tab-label">{tab.label}</span>
        {#if tab.dirty}<span class="tab-dirty" aria-hidden="true"></span>{/if}
      </button>
      <button
        class="tab-close"
        aria-label="Close {tab.label}"
        title="Close"
        onclick={(e) => {
          e.stopPropagation();
          onclose?.(tab.key);
        }}
      >
        <X size={12} strokeWidth={2.25} aria-hidden="true" />
      </button>
    </div>
  {/each}
</div>

<style>
  .tabbar {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    height: var(--bar-h, 38px);
    padding: 0 0.3rem;
    overflow-x: auto;
    overflow-y: hidden;
    background: transparent;
    scrollbar-width: thin;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    flex: none;
    max-width: 14rem;
    height: 1.65rem;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
  }
  .tab:hover {
    background: var(--surface-2);
    color: var(--ink-soft);
  }
  .tab.active {
    background: var(--surface-3);
    border-color: var(--line-strong);
    color: var(--ink);
  }
  .tab-open {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    padding: 0 0.2rem 0 0.55rem;
    height: 100%;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
  }
  .tab-open :global(.tab-ico) {
    flex: none;
    opacity: 0.85;
  }
  .tab-label {
    font-family: var(--font-mono);
    font-size: 0.74rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tab-dirty {
    flex: none;
    width: 0.4rem;
    height: 0.4rem;
    border-radius: 999px;
    background: var(--warn);
  }
  .tab-close {
    display: grid;
    place-items: center;
    width: 1.2rem;
    height: 1.2rem;
    margin-right: 0.2rem;
    border-radius: var(--radius-sm);
    background: transparent;
    border: none;
    color: var(--ink-faint);
    cursor: pointer;
    opacity: 0;
  }
  .tab:hover .tab-close,
  .tab.active .tab-close {
    opacity: 1;
  }
  .tab-close:hover {
    background: var(--surface);
    color: var(--ink);
  }
</style>
