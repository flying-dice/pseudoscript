<script lang="ts">
  import { untrack } from "svelte";
  import type { Component as ComponentType } from "svelte";

  import { Box, Component, Container, Database, FlaskConical, SquareFunction, User } from "@lucide/svelte";

  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";

  // A C4 node kind — one of the six structural levels — or a `feature` block.
  type NodeKind = "person" | "system" | "container" | "component" | "data" | "callable" | "feature";

  // A declared node, nested by structural `parent` into the symbol tree.
  type SymbolNode = {
    fqn: string;
    name: string;
    kind: NodeKind;
    parent?: string | null;
    triggered?: boolean;
    [key: string]: unknown;
  };

  type Props = {
    // Every declared node, nested by structural `parent` (a node's parent may
    // live in another module, so this is workspace-wide, not per-file).
    symbols?: SymbolNode[];
    selectedFqn?: string | null;
    // Bump to force-reveal the selected row even after the user manually
    // collapsed its ancestors (the auto-reveal fires only on selection change).
    revealSignal?: number;
    onpicknode?: (fqn: string) => void;
    // "Go to definition" — always opens the source, distinct from a left-click pick
    // (which is view-aware, e.g. re-targets the 3D graph).
    ongotodef?: (fqn: string) => void;
    onreveal?: (fqn: string) => void;
    onshowuniverse?: (fqn: string) => void;
  };

  let { symbols = [], selectedFqn = null, revealSignal = 0, onpicknode, ongotodef, onreveal, onshowuniverse }: Props = $props();

  // One icon per C4 level, so a node's place in the hierarchy reads at a glance.
  const ICONS: Record<NodeKind, ComponentType> = {
    person: User,
    system: Box,
    container: Container,
    component: Component,
    data: Database,
    callable: SquareFunction,
    feature: FlaskConical,
  };

  // Collapsed subtrees, by node FQN. Default expanded.
  let collapsed = $state(new Set<string>());
  function toggle(fqn: string): void {
    const next = new Set(collapsed);
    next.has(fqn) ? next.delete(fqn) : next.add(fqn);
    collapsed = next;
  }

  // Nest the flat node list by structural `parent` (a node whose parent isn't in
  // the set — a top-level person/system/data — is a root).
  const tree = $derived.by(() => {
    const byFqn = new Map<string, SymbolNode>(symbols.map((n) => [n.fqn, n]));
    const children = new Map<string, SymbolNode[]>();
    const roots: SymbolNode[] = [];
    for (const n of symbols) {
      if (n.parent && byFqn.has(n.parent)) {
        if (!children.has(n.parent)) children.set(n.parent, []);
        children.get(n.parent)!.push(n);
      } else {
        roots.push(n);
      }
    }
    const order: Record<NodeKind, number> = { person: 0, system: 1, container: 2, component: 3, data: 4, callable: 5, feature: 6 };
    const sort = (list: SymbolNode[]): SymbolNode[] =>
      [...list].sort((a, b) => order[a.kind] - order[b.kind] || a.name.localeCompare(b.name));
    for (const [, list] of children) sort(list);
    return { roots: sort(roots), children };
  });

  // The rendered row button per node fqn, bound by the snippet — reveal scrolls
  // the target row without querying its own rendered DOM.
  const rowEls: Record<string, HTMLElement | undefined> = {};

  // Expand every ancestor of `fqn` so its row can render.
  function expandAncestors(fqn: string): void {
    const byFqn = new Map(symbols.map((n) => [n.fqn, n]));
    const next = new Set(collapsed);
    for (let cur = byFqn.get(fqn); cur?.parent && byFqn.has(cur.parent); cur = byFqn.get(cur.parent)) next.delete(cur.parent);
    collapsed = next;
  }

  // Expand, then scroll the row into view once the re-render has materialised
  // it (rAF — the row may not exist yet while an ancestor is collapsed).
  function reveal(fqn: string): void {
    expandAncestors(fqn);
    requestAnimationFrame(() => rowEls[fqn]?.scrollIntoView({ block: "nearest" }));
  }

  // Follow the shared selection (canvas / 3D / editor clicks land here). Tracks
  // only `selectedFqn`: `untrack` keeps `collapsed` out of the dependency set,
  // so a manual collapse after the reveal sticks until the selection changes.
  $effect(() => {
    const fqn = selectedFqn;
    if (fqn) untrack(() => reveal(fqn));
  });

  // Explicit reveal (the panel-header button): re-runs even when the selection
  // is unchanged, overriding manual collapses.
  $effect(() => {
    if (revealSignal === 0) return; // mount — nothing requested yet
    untrack(() => {
      if (selectedFqn) reveal(selectedFqn);
    });
  });
</script>

{#if tree.roots.length === 0}
  <div class="empty">No nodes declared yet.</div>
{:else}
  <ul class="symbols">
    {#each tree.roots as node (node.fqn)}{@render row(node, 0, tree.children)}{/each}
  </ul>
{/if}

{#snippet row(node: SymbolNode, depth: number, children: Map<string, SymbolNode[]>)}
  {@const kids = children.get(node.fqn) ?? []}
  {@const Icon = ICONS[node.kind] ?? Box}
  {@const open = !collapsed.has(node.fqn)}
  <li>
    <div class="row" style="--depth: {depth}">
      <button
        class="twist"
        class:open
        disabled={kids.length === 0}
        aria-label={open ? "Collapse" : "Expand"}
        aria-expanded={open}
        data-testid="twist-{node.fqn}"
        onclick={() => toggle(node.fqn)}
      >▸</button>
      <ContextMenu.Root>
        <ContextMenu.Trigger class="node-trigger">
          <button
            class="node kind-{node.kind}"
            class:active={node.fqn === selectedFqn}
            bind:this={rowEls[node.fqn]}
            onclick={() => onpicknode?.(node.fqn)}
            title="{node.kind} · {node.fqn}"
            data-testid="symbol-{node.fqn}"
          >
            <Icon class="ico" size={14} strokeWidth={1.75} aria-hidden="true" />
            <span class="label">{node.name}</span>
            {#if node.triggered}<span class="trig" title="Triggered callable">▸</span>{/if}
          </button>
        </ContextMenu.Trigger>
        <ContextMenu.Content class="ctx-menu">
          <ContextMenu.Item data-testid="ctx-goto-definition" onSelect={() => (ongotodef ?? onpicknode)?.(node.fqn)}>Go to definition</ContextMenu.Item>
          {#if onreveal}<ContextMenu.Item data-testid="ctx-reveal-canvas" onSelect={() => onreveal?.(node.fqn)}>Reveal on canvas</ContextMenu.Item>{/if}
          {#if onshowuniverse}<ContextMenu.Item data-testid="ctx-show-universe" onSelect={() => onshowuniverse?.(node.fqn)}>Show in 3D view</ContextMenu.Item>{/if}
        </ContextMenu.Content>
      </ContextMenu.Root>
    </div>
    {#if open && kids.length}
      <ul>
        {#each kids as kid (kid.fqn)}{@render row(kid, depth + 1, children)}{/each}
      </ul>
    {/if}
  </li>
{/snippet}

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .symbols {
    padding: 0 0.4rem;
  }
  .empty {
    padding: 0.3rem 0.7rem;
    font-size: 0.76rem;
    color: var(--ink-faint);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.1rem;
    padding-left: calc(var(--depth) * 0.85rem);
  }
  :global(.node-trigger) {
    flex: 1;
    min-width: 0;
    display: flex;
  }
  .twist {
    flex: none;
    width: 1.05rem;
    height: 1.05rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    color: var(--ink-faint);
    font-size: 0.6rem;
    cursor: pointer;
    transition:
      transform 0.13s,
      color 0.13s;
  }
  .twist.open {
    transform: rotate(90deg);
  }
  .twist:hover:not(:disabled) {
    color: var(--ink);
  }
  .twist:disabled {
    opacity: 0.2;
    cursor: default;
  }
  .node {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.18rem 0.45rem;
    color: var(--ink-soft);
    cursor: pointer;
  }
  .node:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .node.active {
    background: var(--accent-soft);
  }
  .node.active .label {
    color: var(--accent);
  }
  .node :global(.ico) {
    flex: none;
    opacity: 0.9;
  }
  .label {
    font-family: var(--font-mono);
    font-size: 0.77rem;
    color: var(--ink-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .node:hover .label {
    color: var(--ink);
  }
  .trig {
    color: var(--accent);
    font-size: 0.6rem;
  }

  /* kind accents — icon inherits via currentColor */
  .kind-person {
    color: #6e8bff;
  }
  .kind-system {
    color: var(--accent-hi);
  }
  .kind-container {
    color: #2dd4bf;
  }
  .kind-component {
    color: #b87bf5;
  }
  .kind-data {
    color: var(--warn);
  }
  .kind-callable {
    color: var(--ink-faint);
  }
  .kind-feature {
    color: #34d399;
  }
</style>
