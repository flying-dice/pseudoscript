<script lang="ts">
  import ProblemsBadge from "./ProblemsBadge.svelte";

  type Problem = { severity: string; message: string; file?: string; start_line: number; start_col: number; code?: string };

  type Props = {
    ver?: string;
    hasWorkspace?: boolean;
    fileLabel?: string;
    fileDirty?: boolean;
    moduleCount?: number;
    toast?: string | null;
    mode?: string;
    saveState?: "idle" | "saving" | "saved" | "error";
    dirtyCount?: number;
    problems?: Problem[];
    errorCount?: number;
    onproblempick?: (d: Problem) => void;
  };

  let {
    ver = "",
    hasWorkspace = false,
    fileLabel = "—",
    fileDirty = false,
    moduleCount = 0,
    toast = null,
    mode = "",
    saveState = "idle",
    dirtyCount = 0,
    problems = [],
    errorCount = 0,
    onproblempick,
  }: Props = $props();
</script>

<footer class="statusbar">
  <span class="seg accent">pds</span>
  <span class="seg dim">wasm{ver ? ` ${ver}` : ""}</span>
  {#if hasWorkspace}
    <span class="seg file" class:dirty={fileDirty}>
      {#if fileDirty}<span class="unsaved-dot" aria-hidden="true"></span>{/if}
      {fileLabel}
    </span>
    <span class="seg dim">{moduleCount} modules</span>
  {/if}

  <span class="grow"></span>

  {#if toast}<span class="seg toast">{toast}</span>{/if}

  {#if hasWorkspace}
    <span class="seg save" aria-live="polite">
      {#if saveState === "saving"}
        <span class="dot busy"></span>saving…
      {:else if saveState === "error"}
        <span class="dot bad"></span><span class="bad">save failed</span>
      {:else if dirtyCount > 0}
        <span class="dot warn"></span><span class="warn">{dirtyCount} unsaved</span>
      {:else}
        <span class="dot ok"></span>saved
      {/if}
    </span>
  {/if}

  <span class="seg dim">{mode}</span>

  {#if hasWorkspace}
    <ProblemsBadge {problems} {errorCount} onpick={onproblempick} />
  {/if}
</footer>

<style>
  .statusbar {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    height: var(--status-h, 26px);
    padding: 0 0.5rem 0 0.8rem;
    border-top: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--ink-faint);
  }
  .seg {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    white-space: nowrap;
  }
  .seg.accent {
    color: var(--accent);
    font-weight: 600;
  }
  .seg.dim {
    opacity: 0.7;
  }
  .seg.file {
    color: var(--ink-soft);
  }
  .seg.file.dirty {
    color: var(--warn);
  }
  .seg.toast {
    color: var(--accent);
  }
  .seg.save .warn {
    color: var(--warn);
  }
  .seg.save .bad {
    color: var(--err);
  }
  .unsaved-dot,
  .dot {
    width: 0.4rem;
    height: 0.4rem;
    border-radius: 999px;
    background: var(--ink-faint);
  }
  .unsaved-dot,
  .dot.warn {
    background: var(--warn);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.bad {
    background: var(--err);
  }
  .dot.busy {
    background: var(--accent);
    animation: pulse 1s ease-in-out infinite;
  }
  .grow {
    flex: 1;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
