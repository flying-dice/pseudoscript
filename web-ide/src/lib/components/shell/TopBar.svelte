<script lang="ts">
  import { ArrowLeft, ArrowRight, Search, WandSparkles } from "@lucide/svelte";

  import { Button } from "$lib/components/ui/button/index.js";
  import FileMenu from "./FileMenu.svelte";

  type Props = {
    workspaceName?: string | null;
    building?: boolean;
    canBack?: boolean;
    canForward?: boolean;
    onback?: () => void;
    onforward?: () => void;
    onopenfolder?: () => void;
    ongoto?: () => void;
    onnewfile?: () => void;
    onnewdoc?: () => void;
    onsave?: () => void;
    onsaveall?: () => void;
    onshare?: () => void;
    onexport?: () => void;
    onimport?: () => void;
    onbuilddocs?: () => void;
    onformat?: () => void;
  };

  let {
    workspaceName = null,
    building = false,
    canBack = false,
    canForward = false,
    onback,
    onforward,
    onformat,
    ...menu
  }: Props = $props();

  const mod = typeof navigator !== "undefined" && /mac/i.test(navigator.platform) ? "⌘" : "Ctrl";
</script>

<header class="topbar">
  <div class="left">
    <span class="brand">pds</span>
    <FileMenu {workspaceName} {building} {...menu} />
    {#if workspaceName}
      <div class="nav">
        <button class="icon-btn" onclick={onback} disabled={!canBack} title="Back (previous location)" aria-label="Back">
          <ArrowLeft size={15} strokeWidth={2} aria-hidden="true" />
        </button>
        <button class="icon-btn" onclick={onforward} disabled={!canForward} title="Forward (next location)" aria-label="Forward">
          <ArrowRight size={15} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>
    {/if}
  </div>

  {#if workspaceName}
    <button class="search" onclick={() => menu.ongoto?.()} title="Go to file or symbol">
      <Search size={13} strokeWidth={2} aria-hidden="true" />
      <span class="search-label">{workspaceName}</span>
      <kbd>{mod}K</kbd>
    </button>
  {/if}

  <div class="right">
    <Button variant="ghost" size="sm" onclick={onformat} title="Format the active file">
      <WandSparkles size={14} strokeWidth={1.75} aria-hidden="true" />
      Format
    </Button>
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    height: var(--bar-h, 34px);
    padding: 0 0.5rem 0 0.7rem;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 75%, transparent);
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 0.1rem;
    margin-left: 0.2rem;
  }
  .brand {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 0.92rem;
    color: var(--accent);
    letter-spacing: 0.02em;
    padding-right: 0.2rem;
  }
  /* the VS-Code-style centre "go to" pill */
  .search {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 16rem;
    max-width: 26rem;
    height: 1.55rem;
    padding: 0 0.5rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    cursor: pointer;
  }
  .search:hover {
    border-color: var(--accent);
    color: var(--ink-soft);
  }
  .search-label {
    flex: 1;
    text-align: center;
    font-family: var(--font-sans);
    font-size: 0.76rem;
    color: var(--ink-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .search kbd {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    padding: 0.05rem 0.3rem;
    border-radius: 4px;
    background: var(--surface-3);
    color: var(--ink-faint);
  }
  .icon-btn {
    width: 1.7rem;
    height: 1.7rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    cursor: pointer;
    text-decoration: none;
    font-size: 0.85rem;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--ink);
  }
  .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
