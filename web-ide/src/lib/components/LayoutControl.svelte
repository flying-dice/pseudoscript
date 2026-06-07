<script lang="ts">
  // Per-diagram layout tweaks for the C4 canvas: a small toolbar dropdown that
  // toggles the long-edge optimiser, the reading direction, and node spacing.
  // Emits the full tweaks object on every change; the page persists it per
  // diagram and re-requests the layout. Mirrors DiagramExport's trigger styling
  // (the shared `.export-trigger` class) so the two read as one toolbar.
  import { SlidersHorizontal } from "@lucide/svelte";

  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import type { LayoutTweaks } from "$lib/core/types.js";

  type Props = {
    tweaks: LayoutTweaks;
    onchange: (tweaks: LayoutTweaks) => void;
  };

  let { tweaks, onchange }: Props = $props();

  const set = (patch: Partial<LayoutTweaks>): void => onchange({ ...tweaks, ...patch });

  // The grid-placement cost dials, shown when grid placement is on. Each maps to a
  // weight in the engine's single cost function (crates/pseudoscript-dot grid.rs).
  const dials = [
    {
      key: "gridFlowCost",
      label: "Directionality",
      hint: "Pull arrows along the reading direction",
      max: 15,
    },
    {
      key: "gridCrossingCost",
      label: "Avoid crossings",
      hint: "Penalty for an edge over a node or another edge",
      max: 30,
    },
    {
      key: "gridDistanceCost",
      label: "Shorten edges",
      hint: "Penalty per cell of edge length",
      max: 10,
    },
  ] as const satisfies ReadonlyArray<{
    key: "gridFlowCost" | "gridCrossingCost" | "gridDistanceCost";
    label: string;
    hint: string;
    max: number;
  }>;
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger class="export-trigger" aria-label="Layout options">
    <SlidersHorizontal size={13} strokeWidth={2} aria-hidden="true" />
    Layout
  </DropdownMenu.Trigger>
  <DropdownMenu.Content align="end" sideOffset={4}>
    <DropdownMenu.CheckboxItem
      checked={tweaks.minimizeLongEdges}
      onCheckedChange={(v) => set({ minimizeLongEdges: v === true })}
    >
      Minimise long edges
    </DropdownMenu.CheckboxItem>

    <DropdownMenu.Separator />
    <DropdownMenu.Label>Orientation</DropdownMenu.Label>
    <DropdownMenu.RadioGroup
      value={tweaks.orientation}
      onValueChange={(v) => set({ orientation: v as LayoutTweaks["orientation"] })}
    >
      <DropdownMenu.RadioItem value="tb">Top to bottom</DropdownMenu.RadioItem>
      <DropdownMenu.RadioItem value="lr">Left to right</DropdownMenu.RadioItem>
    </DropdownMenu.RadioGroup>

    <DropdownMenu.Separator />
    <DropdownMenu.Label>Spacing</DropdownMenu.Label>
    <DropdownMenu.RadioGroup
      value={tweaks.spacing}
      onValueChange={(v) => set({ spacing: v as LayoutTweaks["spacing"] })}
    >
      <DropdownMenu.RadioItem value="compact">Compact</DropdownMenu.RadioItem>
      <DropdownMenu.RadioItem value="comfortable">Comfortable</DropdownMenu.RadioItem>
      <DropdownMenu.RadioItem value="roomy">Roomy</DropdownMenu.RadioItem>
    </DropdownMenu.RadioGroup>

    <DropdownMenu.Separator />
    <DropdownMenu.Label>Placement</DropdownMenu.Label>
    <DropdownMenu.CheckboxItem
      checked={tweaks.experimentalGrid}
      closeOnSelect={false}
      onCheckedChange={(v) => set({ experimentalGrid: v === true })}
    >
      Grid placement
    </DropdownMenu.CheckboxItem>

    {#if tweaks.experimentalGrid}
      <!-- Cost dials: plain content, not menu items, so the menu stays open and
           the sliders own their own pointer/keyboard interaction. -->
      <div
        class="grid gap-2.5 px-2 py-2"
        role="group"
        aria-label="Grid cost dials"
        onkeydowncapture={(e) => e.stopPropagation()}
      >
        {#each dials as dial (dial.key)}
          <label class="grid gap-1 text-xs">
            <span class="flex items-center justify-between">
              <span class="text-muted-foreground" title={dial.hint}>{dial.label}</span>
              <span class="tabular-nums font-medium">{tweaks[dial.key]}</span>
            </span>
            <input
              type="range"
              class="h-1.5 w-full cursor-pointer accent-primary"
              min="0"
              max={dial.max}
              step="1"
              value={tweaks[dial.key]}
              oninput={(e) => set({ [dial.key]: e.currentTarget.valueAsNumber })}
            />
          </label>
        {/each}
      </div>

      <DropdownMenu.Separator />
      <DropdownMenu.Label>Search</DropdownMenu.Label>
      <DropdownMenu.RadioGroup
        value={tweaks.gridSearch}
        onValueChange={(v) => set({ gridSearch: v as LayoutTweaks["gridSearch"] })}
      >
        <DropdownMenu.RadioItem value="auto">Auto</DropdownMenu.RadioItem>
        <DropdownMenu.RadioItem value="heuristic">Heuristic</DropdownMenu.RadioItem>
        <DropdownMenu.RadioItem value="exhaustive">Exact (brute force)</DropdownMenu.RadioItem>
      </DropdownMenu.RadioGroup>
    {/if}
  </DropdownMenu.Content>
</DropdownMenu.Root>
