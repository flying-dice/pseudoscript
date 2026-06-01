<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";

  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";

  type Props = {
    workspaceName?: string | null;
    building?: boolean;
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
  };

  let {
    workspaceName = null,
    building = false,
    onopenfolder,
    ongoto,
    onnewfile,
    onnewdoc,
    onsave,
    onsaveall,
    onshare,
    onexport,
    onimport,
    onbuilddocs,
  }: Props = $props();

  // The platform-correct modifier glyph for the shortcut hints.
  const mod = typeof navigator !== "undefined" && /mac/i.test(navigator.platform) ? "⌘" : "Ctrl";

  const hasWorkspace = $derived(!!workspaceName);
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger class="file-menu-trigger">
    File
    <ChevronDown size={13} strokeWidth={2} aria-hidden="true" />
  </DropdownMenu.Trigger>
  <DropdownMenu.Content class="file-menu" align="start" sideOffset={4}>
    <DropdownMenu.Item onSelect={() => onopenfolder?.()}>Open project…</DropdownMenu.Item>
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => ongoto?.()}>
      Go to file or symbol…
      <DropdownMenu.Shortcut>{mod}K</DropdownMenu.Shortcut>
    </DropdownMenu.Item>
    <DropdownMenu.Separator />
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onnewfile?.()}>New file…</DropdownMenu.Item>
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onnewdoc?.()}>New doc…</DropdownMenu.Item>
    <DropdownMenu.Separator />
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onsave?.()}>
      Save
      <DropdownMenu.Shortcut>{mod}S</DropdownMenu.Shortcut>
    </DropdownMenu.Item>
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onsaveall?.()}>Save all</DropdownMenu.Item>
    <DropdownMenu.Separator />
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onshare?.()}>Copy share link</DropdownMenu.Item>
    <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onexport?.()}>Export .pdsx</DropdownMenu.Item>
    <DropdownMenu.Item onSelect={() => onimport?.()}>Import .pdsx…</DropdownMenu.Item>
    <DropdownMenu.Separator />
    <DropdownMenu.Item disabled={building} onSelect={() => onbuilddocs?.()}>
      {building ? "Building…" : "Build docs"}
    </DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>

<style>
  :global(.file-menu-trigger) {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    height: 1.6rem;
    padding: 0 0.55rem;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    font-family: var(--font-sans);
    font-size: 0.82rem;
    cursor: pointer;
  }
  :global(.file-menu-trigger:hover),
  :global(.file-menu-trigger[data-state="open"]) {
    background: var(--surface-2);
    color: var(--ink);
  }
  :global(.file-menu) {
    min-width: 12rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    padding: 0.3rem;
    font-family: var(--font-sans);
    color: var(--ink-soft);
  }
</style>
