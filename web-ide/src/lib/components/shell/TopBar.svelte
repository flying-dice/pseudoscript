<script lang="ts">
  import { Monitor, Moon, Search, Sun, WandSparkles } from "@lucide/svelte";

  import { Button } from "$lib/components/ui/button/index.js";
  import { theme, THEME_OPTIONS } from "$lib/theme.svelte.js";
  import type { ThemePref } from "$lib/theme.svelte.js";
  import FileMenu from "./FileMenu.svelte";
  import ProblemsBadge from "./ProblemsBadge.svelte";

  type Problem = { severity: string; message: string; file?: string; start_line: number; start_col: number; code?: string };

  type Props = {
    workspaceName?: string | null;
    building?: boolean;
    base?: string;
    saveState?: "idle" | "saving" | "saved" | "error";
    dirtyCount?: number;
    problems?: Problem[];
    errorCount?: number;
    onproblempick?: (d: Problem) => void;
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
    base = "",
    saveState = "idle",
    dirtyCount = 0,
    problems = [],
    errorCount = 0,
    onproblempick,
    onformat,
    ...menu
  }: Props = $props();

  const THEME_ICON: Record<ThemePref, typeof Monitor> = { system: Monitor, light: Sun, dark: Moon };
  const THEME_LABEL: Record<ThemePref, string> = { system: "System", light: "Light", dark: "Dark" };
  const ThemeIcon = $derived(THEME_ICON[theme.pref]);
  function cycleTheme(): void {
    const i = THEME_OPTIONS.indexOf(theme.pref);
    theme.set(THEME_OPTIONS[(i + 1) % THEME_OPTIONS.length]);
  }

  const mod = typeof navigator !== "undefined" && /mac/i.test(navigator.platform) ? "⌘" : "Ctrl";
</script>

<header class="topbar">
  <div class="left">
    <span class="brand">pds</span>
    <FileMenu {workspaceName} {building} {...menu} />
  </div>

  {#if workspaceName}
    <button class="search" onclick={() => menu.ongoto?.()} title="Go to file or symbol">
      <Search size={13} strokeWidth={2} aria-hidden="true" />
      <span class="search-label">{workspaceName}</span>
      <kbd>{mod}K</kbd>
    </button>
  {/if}

  <div class="right">
    {#if workspaceName}
      <div class="save" aria-live="polite">
        {#if saveState === "saving"}
          <span class="dot busy"></span><span class="lbl">saving…</span>
        {:else if saveState === "error"}
          <span class="dot bad"></span><span class="lbl bad">save failed</span>
        {:else if dirtyCount > 0}
          <span class="dot warn"></span><span class="lbl warn">{dirtyCount} unsaved</span>
        {:else}
          <span class="dot ok"></span><span class="lbl">saved</span>
        {/if}
      </div>
    {/if}

    <Button variant="ghost" size="sm" onclick={onformat} title="Format the active file">
      <WandSparkles size={14} strokeWidth={1.75} aria-hidden="true" />
      Format
    </Button>

    <button class="icon-btn" onclick={cycleTheme} title={`Theme: ${THEME_LABEL[theme.pref]}`} aria-label={`Theme: ${THEME_LABEL[theme.pref]}`}>
      <ThemeIcon size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>

    <a
      class="icon-btn"
      href="{base}/pseudocode-skill.zip"
      download="pseudocode-skill.zip"
      title="Download the PseudoScript authoring skill (.zip)"
      aria-label="Download the authoring skill"
      data-testid="download-skill"
    >
      <span aria-hidden="true">📥</span>
    </a>

    <ProblemsBadge {problems} {errorCount} onpick={onproblempick} />
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
  .icon-btn:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .save {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--ink-faint);
  }
  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 999px;
    background: var(--ink-faint);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.bad {
    background: var(--err);
  }
  .dot.busy {
    background: var(--accent);
    animation: pulse 1s ease-in-out infinite;
  }
  .lbl.warn {
    color: var(--warn);
  }
  .lbl.bad {
    color: var(--err);
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
