<script lang="ts">
  type Props = {
    ver?: string;
    hasWorkspace?: boolean;
    fileLabel?: string;
    fileDirty?: boolean;
    moduleCount?: number;
    toast?: string | null;
    mode?: string;
  };

  let { ver = "", hasWorkspace = false, fileLabel = "—", fileDirty = false, moduleCount = 0, toast = null, mode = "" }: Props = $props();
</script>

<footer class="statusbar">
  <span class="seg accent">pds</span>
  <span class="seg">wasm{ver ? ` ${ver}` : ""}</span>
  {#if hasWorkspace}
    <span class="seg file" class:dirty={fileDirty}>
      {#if fileDirty}<span class="unsaved-dot" aria-hidden="true"></span>{/if}
      {fileLabel}
    </span>
    <span class="seg dim">{moduleCount} modules</span>
  {/if}
  <span class="grow"></span>
  {#if toast}<span class="seg toast">{toast}</span>{/if}
  <span class="seg dim">{mode}</span>
</footer>

<style>
  .statusbar {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    height: var(--status-h, 26px);
    padding: 0 0.8rem;
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
  .unsaved-dot {
    width: 0.4rem;
    height: 0.4rem;
    border-radius: 999px;
    background: var(--warn);
  }
  .grow {
    flex: 1;
  }
</style>
