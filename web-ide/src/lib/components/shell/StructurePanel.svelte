<script lang="ts">
  import { PanelRightClose, Search, X } from "@lucide/svelte";

  import SymbolTree from "./SymbolTree.svelte";

  type SymbolNode = { fqn: string; name: string; kind: string; parent?: string | null; triggered?: boolean; [key: string]: unknown };

  type Props = {
    symbols?: SymbolNode[];
    selectedFqn?: string | null;
    onpicknode?: (fqn: string) => void;
    onreveal?: (fqn: string) => void;
    oncollapse?: () => void;
  };

  let { symbols = [], selectedFqn = null, onpicknode, onreveal, oncollapse }: Props = $props();

  let query = $state("");

  // Filter to nodes matching the query, keeping their ancestors so the tree still
  // nests. Empty query shows everything.
  const filtered = $derived.by<SymbolNode[]>(() => {
    const q = query.trim().toLowerCase();
    if (!q) return symbols;
    const byFqn = new Map(symbols.map((n) => [n.fqn, n]));
    const keep = new Set<string>();
    for (const n of symbols) {
      if (n.name.toLowerCase().includes(q) || n.fqn.toLowerCase().includes(q)) {
        let cur: SymbolNode | undefined = n;
        while (cur && !keep.has(cur.fqn)) {
          keep.add(cur.fqn);
          cur = cur.parent ? byFqn.get(cur.parent) : undefined;
        }
      }
    }
    return symbols.filter((n) => keep.has(n.fqn));
  });
</script>

<aside class="structure island">
  <header class="panel-head">
    <span class="title">Structure</span>
    <span class="count">{filtered.length}</span>
    <button class="collapse" onclick={oncollapse} aria-label="Collapse structure" title="Collapse structure">
      <PanelRightClose size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>
  </header>
  <div class="search">
    <Search size={13} strokeWidth={2} aria-hidden="true" />
    <input type="text" placeholder="Filter symbols…" bind:value={query} aria-label="Filter symbols" spellcheck="false" autocomplete="off" />
    {#if query}
      <button class="clear" onclick={() => (query = "")} aria-label="Clear filter" title="Clear">
        <X size={12} strokeWidth={2.25} aria-hidden="true" />
      </button>
    {/if}
  </div>
  <div class="panel-body">
    {#if filtered.length === 0 && query}
      <div class="empty">No symbol matches “{query}”.</div>
    {:else}
      <SymbolTree symbols={filtered as never} {selectedFqn} {onpicknode} {onreveal} />
    {/if}
  </div>
</aside>

<style>
  .structure {
    display: grid;
    grid-template-rows: var(--bar-h, 34px) auto 1fr;
    min-height: 0;
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }
  .panel-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: var(--bar-h, 34px);
    padding: 0 0.4rem 0 0.7rem;
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    color: var(--ink-faint);
  }
  .collapse {
    margin-left: auto;
    width: 1.5rem;
    height: 1.5rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    cursor: pointer;
  }
  .collapse:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: 0.4rem 0.5rem;
    padding: 0 0.45rem;
    height: 1.55rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.74rem;
  }
  .search input::placeholder {
    color: var(--ink-faint);
  }
  .clear {
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    color: var(--ink-faint);
    cursor: pointer;
  }
  .clear:hover {
    color: var(--ink);
  }
  .panel-body {
    min-height: 0;
    overflow: auto;
    padding: 0.2rem 0 0.4rem;
  }
  .empty {
    padding: 0.4rem 0.7rem;
    font-size: 0.76rem;
    color: var(--ink-faint);
  }
</style>
