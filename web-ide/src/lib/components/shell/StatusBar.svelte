<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    // Left-aligned status content — the model breadcrumb lives here.
    children?: Snippet;
  };

  let { children }: Props = $props();
</script>

<footer class="statusbar">
  {@render children?.()}
</footer>

<style>
  .statusbar {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    height: var(--status-h, 26px);
    padding: 0 0.6rem 0 0.5rem;
    background: none;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--ink-faint);
  }
</style>
