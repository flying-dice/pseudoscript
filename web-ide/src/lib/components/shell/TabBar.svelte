<script lang="ts">
  import { File, FileCode, FileText, Settings2, X } from "@lucide/svelte";
  import type { Component as ComponentType } from "svelte";

  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";

  type Tab = { key: string; label: string; kind: "module" | "doc" | "manifest" | "other"; active: boolean; dirty: boolean };

  type Props = {
    tabs?: Tab[];
    onselect?: (key: string) => void;
    onclose?: (key: string) => void;
    oncloseothers?: (key: string) => void;
    onclosetoright?: (key: string) => void;
    oncloseall?: () => void;
    onreorder?: (fromKey: string, toKey: string) => void;
  };

  let { tabs = [], onselect, onclose, oncloseothers, onclosetoright, oncloseall, onreorder }: Props = $props();

  const ICON: Record<Tab["kind"], ComponentType> = { module: FileCode, doc: FileText, manifest: Settings2, other: File };

  // Native HTML5 drag-to-reorder (same approach as FileTree's file moves).
  let dragKey = $state<string | null>(null);
  let dropKey = $state<string | null>(null);
</script>

<div class="tabbar" role="tablist" aria-label="Open files">
  {#each tabs as tab, index (tab.key)}
    {@const Icon = ICON[tab.kind]}
    <ContextMenu.Root>
      <ContextMenu.Trigger class="tab-trigger">
        <div
          class="tab"
          class:active={tab.active}
          class:drop-target={dropKey === tab.key && dragKey !== tab.key}
          role="presentation"
          draggable={true}
          ondragstart={() => (dragKey = tab.key)}
          ondragend={() => {
            dragKey = null;
            dropKey = null;
          }}
          ondragover={(e) => {
            if (dragKey) {
              e.preventDefault();
              dropKey = tab.key;
            }
          }}
          ondrop={(e) => {
            e.preventDefault();
            if (dragKey && dragKey !== tab.key) onreorder?.(dragKey, tab.key);
            dragKey = null;
            dropKey = null;
          }}
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
      </ContextMenu.Trigger>
      <ContextMenu.Content class="ctx-menu">
        <ContextMenu.Item onSelect={() => onclose?.(tab.key)}>Close</ContextMenu.Item>
        <ContextMenu.Item disabled={tabs.length <= 1} onSelect={() => oncloseothers?.(tab.key)}>Close Others</ContextMenu.Item>
        <ContextMenu.Item disabled={index >= tabs.length - 1} onSelect={() => onclosetoright?.(tab.key)}>Close to the Right</ContextMenu.Item>
        <ContextMenu.Separator />
        <ContextMenu.Item onSelect={() => oncloseall?.()}>Close All</ContextMenu.Item>
      </ContextMenu.Content>
    </ContextMenu.Root>
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
  /* The context-menu trigger must not introduce a box into the flex row. */
  :global(.tab-trigger) {
    display: contents;
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
  .tab.drop-target {
    box-shadow: inset 2px 0 0 0 var(--line-strong);
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
