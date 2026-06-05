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
  </DropdownMenu.Content>
</DropdownMenu.Root>
