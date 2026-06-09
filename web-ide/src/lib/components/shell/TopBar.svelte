<script lang="ts">
  import { ArrowLeft, ArrowRight, Search } from "@lucide/svelte";

  import MenuBar from "./MenuBar.svelte";

  type Props = {
    workspaceName?: string | null;
    building?: boolean;
    view?: "code" | "canvas" | "space";
    structureOpen?: boolean;
    canBack?: boolean;
    canForward?: boolean;
    onback?: () => void;
    onforward?: () => void;
    onopenfolder?: () => void;
    oncloseproject?: () => void;
    ongoto?: () => void;
    onnewfile?: () => void;
    onnewdoc?: () => void;
    onsave?: () => void;
    onsaveall?: () => void;
    onshare?: () => void;
    onexport?: () => void;
    onimport?: () => void;
    onbuilddocs?: () => void;
    onshortcuts?: () => void;
    onview?: (view: "code" | "canvas" | "space") => void;
    ontogglestructure?: () => void;
  };

  let {
    workspaceName = null,
    building = false,
    view = "code",
    structureOpen = true,
    canBack = false,
    canForward = false,
    onback,
    onforward,
    ...menu
  }: Props = $props();

  const mod = typeof navigator !== "undefined" && /mac/i.test(navigator.platform) ? "⌘" : "Ctrl";
</script>

<header class="topbar">
  <div class="left">
    <span class="brand">pds</span>
    <MenuBar {workspaceName} {building} {view} {structureOpen} {canBack} {canForward} {onback} {onforward} {...menu} />
    {#if workspaceName}
      <div class="nav">
        <button class="icon-btn" onclick={onback} disabled={!canBack} title="Back (previous location)" aria-label="Back" data-testid="nav-back">
          <ArrowLeft size={18} strokeWidth={1.75} aria-hidden="true" />
        </button>
        <button class="icon-btn" onclick={onforward} disabled={!canForward} title="Forward (next location)" aria-label="Forward" data-testid="nav-forward">
          <ArrowRight size={18} strokeWidth={1.75} aria-hidden="true" />
        </button>
      </div>
    {/if}
  </div>

  <div class="right">
    {#if workspaceName}
      <button class="icon-btn" onclick={() => menu.ongoto?.()} title="Go to file or symbol ({mod}K)" aria-label="Go to file or symbol">
        <Search size={18} strokeWidth={1.75} aria-hidden="true" />
      </button>
    {/if}
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    height: var(--topbar-h, 42px);
    padding: 0 var(--island-gap) 0 0.7rem;
    background: none;
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  /* Sit the search button over the right rail below it: occupy a rail-width zone
     ending the same island-gap from the edge as the rail column, icon centred. */
  .right {
    width: var(--right-rail-w);
    justify-content: center;
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
  .icon-btn {
    width: 2rem;
    height: 2rem;
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

  /* Installed PWA with Window Controls Overlay: the OS draws the window buttons
     over the bar (traffic lights on the left on macOS), hiding "pds" and the
     File menu. Inset the bar past the reserved titlebar area on both edges and
     make its empty space drag the window; interactive elements opt back out of
     dragging. The right inset reserves space for controls drawn on the right
     edge (Windows/Linux place them there; Chrome also reserves a right region).
     The env() vars are only non-default in this display mode, so the bar is
     unchanged when run in a normal browser tab. */
  @media (display-mode: window-controls-overlay) {
    .topbar {
      padding-left: calc(env(titlebar-area-x, 0px) + 0.7rem);
      padding-right: calc(100vw - env(titlebar-area-x, 0px) - env(titlebar-area-width, 100vw) + var(--island-gap));
      -webkit-app-region: drag;
    }
    .brand,
    .nav,
    .icon-btn,
    :global(.topbar .menu-trigger) {
      -webkit-app-region: no-drag;
    }
  }
</style>
