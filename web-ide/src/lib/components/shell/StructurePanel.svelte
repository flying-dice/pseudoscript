<script lang="ts">
  import { LocateFixed, Search, X } from "@lucide/svelte";

  import SymbolTree from "./SymbolTree.svelte";

  type SymbolNode = { fqn: string; name: string; kind: string; parent?: string | null; triggered?: boolean; [key: string]: unknown };

  type Props = {
    symbols?: SymbolNode[];
    selectedFqn?: string | null;
    onpicknode?: (fqn: string) => void;
    ongotodef?: (fqn: string) => void;
    onreveal?: (fqn: string) => void;
    onshowuniverse?: (fqn: string) => void;
  };

  let { symbols = [], selectedFqn = null, onpicknode, ongotodef, onreveal, onshowuniverse }: Props = $props();

  let query = $state("");

  // Bumped by the header button to force-reveal the selected row in the tree.
  let revealSignal = $state(0);

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

<aside class="structure island" data-testid="structure-panel">
  <div class="head">
    <div class="search">
      <Search size={13} strokeWidth={2} aria-hidden="true" />
      <input type="text" placeholder="Filter symbols…" bind:value={query} aria-label="Filter symbols" spellcheck="false" autocomplete="off" data-testid="structure-filter" />
      {#if query}
        <button class="clear" onclick={() => (query = "")} aria-label="Clear filter" title="Clear" data-testid="structure-filter-clear">
          <X size={12} strokeWidth={2.25} aria-hidden="true" />
        </button>
      {/if}
    </div>
    <button
      class="head-btn"
      title="Reveal current item"
      aria-label="Reveal the selected symbol in the tree"
      disabled={selectedFqn == null}
      onclick={() => revealSignal++}
      data-testid="structure-reveal"
    >
      <LocateFixed size={14} strokeWidth={1.9} aria-hidden="true" />
    </button>
  </div>
  <div class="panel-body">
    {#if filtered.length === 0 && query}
      <div class="empty" data-testid="structure-no-match">No symbol matches “{query}”.</div>
    {:else}
      <SymbolTree symbols={filtered as never} {selectedFqn} {revealSignal} {onpicknode} {ongotodef} {onreveal} {onshowuniverse} />
    {/if}
  </div>
</aside>

<style>
  .structure {
    display: grid;
    grid-template-rows: auto 1fr;
    min-height: 0;
    background: var(--island-bg);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    margin: 0.4rem 0.5rem;
  }
  .search {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.35rem;
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
