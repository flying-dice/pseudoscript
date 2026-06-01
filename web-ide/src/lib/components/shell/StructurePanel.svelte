<script lang="ts">
  import { PanelRightClose } from "@lucide/svelte";

  import SymbolTree from "./SymbolTree.svelte";

  type SymbolNode = { fqn: string; name: string; kind: string; parent?: string | null; triggered?: boolean; [key: string]: unknown };

  type Props = {
    symbols?: SymbolNode[];
    selectedFqn?: string | null;
    onpicknode?: (fqn: string) => void;
    oncollapse?: () => void;
  };

  let { symbols = [], selectedFqn = null, onpicknode, oncollapse }: Props = $props();
</script>

<aside class="structure island">
  <header class="panel-head">
    <span class="title">Structure</span>
    <span class="count">{symbols.length}</span>
    <button class="collapse" onclick={oncollapse} aria-label="Collapse structure" title="Collapse structure">
      <PanelRightClose size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>
  </header>
  <div class="panel-body">
    <SymbolTree symbols={symbols as never} {selectedFqn} {onpicknode} />
  </div>
</aside>

<style>
  .structure {
    display: grid;
    grid-template-rows: var(--bar-h, 34px) 1fr;
    min-height: 0;
    background: color-mix(in srgb, var(--surface) 70%, transparent);
    border-left: 1px solid var(--line);
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
  .panel-body {
    min-height: 0;
    overflow: auto;
    padding: 0.35rem 0;
  }
</style>
