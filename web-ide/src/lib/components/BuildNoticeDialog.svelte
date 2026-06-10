<script lang="ts">
  // Shown when "Build docs" runs against the in-memory bundled example, which has
  // no folder to write to. Docs build to disk only (the `pds doc` parity path) —
  // this notice explains that and offers to open a real folder (when the File
  // System Access API is available), or cancel. Built on the shadcn Dialog for
  // focus-trap, scroll-lock, and Escape handling.
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";

  type Props = {
    /** When set, an "Open a folder…" action is offered (FS Access API present). */
    onopenfolder?: () => void;
    oncancel?: () => void;
  };

  let { onopenfolder, oncancel }: Props = $props();

  function onOpenChange(open: boolean) {
    if (!open) oncancel?.();
  }
</script>

<Dialog.Root open onOpenChange={onOpenChange}>
  <Dialog.Content class="sm:max-w-md" data-testid="build-notice">
    <Dialog.Header>
      <Dialog.Title>Docs build to a folder</Dialog.Title>
    </Dialog.Header>
    <div class="prose">
      <p>
        You're working in the bundled <b>example</b>, which lives in memory — there's no folder to
        write the site to.
      </p>
      <p>
        Open a folder as your workspace and Build docs writes the same site the <code>pds doc</code>
        CLI does, under <code>target/doc/</code> — open its <code>index.html</code> to view it.
      </p>
    </div>
    <Dialog.Footer>
      <Button type="button" variant="ghost" size="sm" onclick={() => oncancel?.()}>Cancel</Button>
      {#if onopenfolder}
        <Button type="button" size="sm" onclick={() => onopenfolder?.()}>Open a folder…</Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  .prose {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    font-size: 0.82rem;
    line-height: 1.5;
    color: var(--ink-soft);
  }
  .prose :global(code) {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    padding: 0.05rem 0.3rem;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--ink);
  }
  .prose :global(b) {
    color: var(--ink);
    font-weight: 600;
  }
</style>
