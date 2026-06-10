<script lang="ts">
  import { base } from "$app/paths";

  import { ChevronDown } from "@lucide/svelte";

  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { theme, THEME_OPTIONS } from "$lib/theme.svelte.js";
  import type { ThemePref } from "$lib/theme.svelte.js";

  type Props = {
    workspaceName?: string | null;
    building?: boolean;
    view?: "code" | "canvas" | "space";
    structureOpen?: boolean;
    canBack?: boolean;
    canForward?: boolean;
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
    onaisettings?: () => void;
    onreference?: () => void;
    onback?: () => void;
    onforward?: () => void;
    onview?: (view: "code" | "canvas" | "space") => void;
    ontogglestructure?: () => void;
    perfHud?: boolean;
    ontoggleperfhud?: () => void;
  };

  let {
    workspaceName = null,
    building = false,
    view = "code",
    structureOpen = true,
    canBack = false,
    canForward = false,
    onopenfolder,
    oncloseproject,
    ongoto,
    onnewfile,
    onnewdoc,
    onsave,
    onsaveall,
    onshare,
    onexport,
    onimport,
    onbuilddocs,
    onshortcuts,
    onaisettings,
    onreference,
    onback,
    onforward,
    onview,
    ontogglestructure,
    perfHud = false,
    ontoggleperfhud,
  }: Props = $props();

  // The platform-correct modifier glyph for the shortcut hints.
  const mod = typeof navigator !== "undefined" && /mac/i.test(navigator.platform) ? "⌘" : "Ctrl";

  const hasWorkspace = $derived(!!workspaceName);

  const THEME_LABEL: Record<ThemePref, string> = { system: "System", light: "Light", dark: "Dark" };
</script>

<nav class="menu-bar" aria-label="Application menu">
  <!-- File -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger class="menu-trigger">
      File
      <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Content class="menu-content" align="start" sideOffset={4}>
      <DropdownMenu.Item onSelect={() => onopenfolder?.()}>Open project…</DropdownMenu.Item>
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => oncloseproject?.()}>Close project</DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onnewfile?.()}>New file…</DropdownMenu.Item>
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onnewdoc?.()}>New doc…</DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onsave?.()}>
        Save
        <DropdownMenu.Shortcut>{mod}S</DropdownMenu.Shortcut>
      </DropdownMenu.Item>
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onsaveall?.()}>
        Save all
        <DropdownMenu.Shortcut>{mod}⇧S</DropdownMenu.Shortcut>
      </DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item onSelect={() => onimport?.()}>Import .pdsx…</DropdownMenu.Item>
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onexport?.()}>Export .pdsx</DropdownMenu.Item>
      <DropdownMenu.Item disabled={!hasWorkspace || building} onSelect={() => onbuilddocs?.()}>
        {building ? "Building…" : "Build docs"}
      </DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => onshare?.()}>Copy share link</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  <!-- Go -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger class="menu-trigger">
      Go
      <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Content class="menu-content" align="start" sideOffset={4}>
      <DropdownMenu.Item disabled={!hasWorkspace} onSelect={() => ongoto?.()}>
        Go to file or symbol…
        <DropdownMenu.Shortcut>{mod}K</DropdownMenu.Shortcut>
      </DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item disabled={!canBack} onSelect={() => onback?.()}>Back</DropdownMenu.Item>
      <DropdownMenu.Item disabled={!canForward} onSelect={() => onforward?.()}>Forward</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  <!-- View -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger class="menu-trigger">
      View
      <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Content class="menu-content" align="start" sideOffset={4}>
      <DropdownMenu.RadioGroup value={view} onValueChange={(v) => onview?.(v as "code" | "canvas" | "space")}>
        <DropdownMenu.RadioItem value="code" disabled={!hasWorkspace}>Editor</DropdownMenu.RadioItem>
        <DropdownMenu.RadioItem value="canvas" disabled={!hasWorkspace}>Diagram canvas</DropdownMenu.RadioItem>
        <DropdownMenu.RadioItem value="space" disabled={!hasWorkspace}>3D graph</DropdownMenu.RadioItem>
      </DropdownMenu.RadioGroup>
      <DropdownMenu.Separator />
      <DropdownMenu.CheckboxItem
        checked={structureOpen}
        disabled={!hasWorkspace}
        onCheckedChange={() => ontogglestructure?.()}
      >
        Structure panel
      </DropdownMenu.CheckboxItem>
      <DropdownMenu.CheckboxItem checked={perfHud} onCheckedChange={() => ontoggleperfhud?.()}>
        Performance meter
      </DropdownMenu.CheckboxItem>
      <DropdownMenu.Separator />
      <DropdownMenu.Sub>
        <DropdownMenu.SubTrigger>Theme</DropdownMenu.SubTrigger>
        <DropdownMenu.SubContent class="menu-content">
          <DropdownMenu.RadioGroup value={theme.pref} onValueChange={(v) => theme.set(v as ThemePref)}>
            {#each THEME_OPTIONS as pref (pref)}
              <DropdownMenu.RadioItem value={pref}>{THEME_LABEL[pref]}</DropdownMenu.RadioItem>
            {/each}
          </DropdownMenu.RadioGroup>
        </DropdownMenu.SubContent>
      </DropdownMenu.Sub>
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  <!-- Help -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger class="menu-trigger">
      Help
      <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Content class="menu-content" align="start" sideOffset={4}>
      <DropdownMenu.Item onSelect={() => onreference?.()} data-testid="menu-reference">
        Language reference…
      </DropdownMenu.Item>
      <DropdownMenu.Item onSelect={() => onshortcuts?.()}>Keyboard shortcuts…</DropdownMenu.Item>
      <DropdownMenu.Item onSelect={() => onaisettings?.()}>AI Completion…</DropdownMenu.Item>
      <DropdownMenu.Item>
        {#snippet child({ props })}
          <a
            {...props}
            href="{base}/pseudocode-skill.zip"
            download="pseudocode-skill.zip"
            data-testid="download-skill"
          >
            Download authoring skill (.zip)
          </a>
        {/snippet}
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</nav>

<style>
  .menu-bar {
    display: flex;
    align-items: center;
    gap: 0.05rem;
  }
  :global(.menu-trigger) {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    height: 1.6rem;
    padding: 0 0.5rem;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    font-family: var(--font-sans);
    font-size: 0.82rem;
    cursor: pointer;
  }
  :global(.menu-trigger:hover),
  :global(.menu-trigger[data-state="open"]) {
    background: var(--surface-2);
    color: var(--ink);
  }
  :global(.menu-content) {
    min-width: 12rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    padding: 0.3rem;
    font-family: var(--font-sans);
    color: var(--ink-soft);
  }
  /* The skill-download item is a real anchor; strip its link styling. */
  :global(.menu-content a[data-testid="download-skill"]) {
    color: inherit;
    text-decoration: none;
  }
</style>
