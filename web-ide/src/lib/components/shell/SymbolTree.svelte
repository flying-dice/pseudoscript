<script lang="ts">
  import type { Component as ComponentType } from "svelte";

  import { Box, Component, Container, Database, SquareFunction, User } from "@lucide/svelte";

  // A C4 node kind — one of the six structural levels.
  type NodeKind = "person" | "system" | "container" | "component" | "data" | "callable";

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
    onpicknode?: (fqn: string) => void;
  };

  let { symbols = [], selectedFqn = null, onpicknode }: Props = $props();

  // One icon per C4 level, so a node's place in the hierarchy reads at a glance.
  const ICONS: Record<NodeKind, ComponentType> = {
    person: User,
    system: Box,
    container: Container,
    component: Component,
    data: Database,
    callable: SquareFunction,
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
    const order: Record<NodeKind, number> = { person: 0, system: 1, container: 2, component: 3, data: 4, callable: 5 };
    const sort = (list: SymbolNode[]): SymbolNode[] =>
      [...list].sort((a, b) => order[a.kind] - order[b.kind] || a.name.localeCompare(b.name));
    for (const [, list] of children) sort(list);
    return { roots: sort(roots), children };
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
        onclick={() => toggle(node.fqn)}
      >▸</button>
      <button
        class="node kind-{node.kind}"
        class:active={node.fqn === selectedFqn}
        onclick={() => onpicknode?.(node.fqn)}
        title="{node.kind} · {node.fqn}"
      >
        <Icon class="ico" size={14} strokeWidth={1.75} aria-hidden="true" />
        <span class="label">{node.name}</span>
        {#if node.triggered}<span class="trig" title="Triggered callable">▸</span>{/if}
      </button>
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
</style>
