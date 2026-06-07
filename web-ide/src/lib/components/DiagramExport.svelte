<script lang="ts">
  // Download the current svelte-flow diagram. A small button floating over the
  // canvas opens a PNG / SVG choice; either captures the whole diagram (re-framed
  // to fit every node, not just the visible viewport) on the themed canvas
  // background and triggers a browser download. Shared by the C4 graph and the
  // sequence timeline — both are svelte-flow canvases.
  import { Download } from "@lucide/svelte";
  import type { Node } from "@xyflow/svelte";

  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { downloadDiagram } from "$lib/flow-export.js";
  import type { ExportFormat } from "$lib/flow-export.js";
  import { notifications } from "$lib/stores/notifications.svelte.js";

  type Props = {
    // The component root holding the `.svelte-flow` canvas (set via bind:this).
    container: HTMLElement | null;
    // The live Svelte Flow nodes, measured, as the canvas holds them.
    nodes: Node[];
    // Download name without extension (e.g. the diagram's subject).
    filename: string;
    // C4 views only: a callback returning the Graphviz `dot` source equivalent
    // to the engine's layout input. When given, a ".dot" download is offered so
    // the graph can be checked against real `dot`.
    dotSource?: (() => string) | null;
  };

  let { container, nodes, filename, dotSource = null }: Props = $props();

  // The concrete canvas background, read off the rendered pane so the export
  // follows the active theme (light / dark) without re-deriving it here.
  function background(root: HTMLElement): string {
    const pane = root.querySelector<HTMLElement>(".svelte-flow");
    const colour = pane ? getComputedStyle(pane).backgroundColor : "";
    return colour && colour !== "rgba(0, 0, 0, 0)" ? colour : "#ffffff";
  }

  async function run(format: ExportFormat): Promise<void> {
    if (!container) return;
    try {
      await downloadDiagram(container, nodes, { format, filename, background: background(container) });
      notifications.notify("success", `Exported ${format.toUpperCase()}`, `${filename}.${format}`);
    } catch (e) {
      notifications.notify("error", "Export failed", String((e as Error)?.message ?? e));
    }
  }

  // Download the Graphviz `dot` source for the current C4 view as a text file.
  function runDot(): void {
    if (!dotSource) return;
    try {
      const blob = new Blob([dotSource()], { type: "text/vnd.graphviz" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${filename}.dot`;
      a.click();
      URL.revokeObjectURL(url);
      notifications.notify("success", "Exported DOT", `${filename}.dot`);
    } catch (e) {
      notifications.notify("error", "Export failed", String((e as Error)?.message ?? e));
    }
  }
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger class="export-trigger" aria-label="Download diagram">
    <Download size={13} strokeWidth={2} aria-hidden="true" />
    Download
  </DropdownMenu.Trigger>
  <DropdownMenu.Content align="end" sideOffset={4}>
    <DropdownMenu.Item onSelect={() => run("png")}>PNG image</DropdownMenu.Item>
    <DropdownMenu.Item onSelect={() => run("svg")}>SVG image</DropdownMenu.Item>
    {#if dotSource}
      <DropdownMenu.Item onSelect={() => runDot()}>Graphviz .dot</DropdownMenu.Item>
    {/if}
  </DropdownMenu.Content>
</DropdownMenu.Root>

<style>
  /* Matches the canvas "Customise" button so the two read as one toolbar. */
  :global(.export-trigger) {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.28rem 0.55rem;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--ink-soft);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    cursor: pointer;
    white-space: nowrap;
  }
  :global(.export-trigger:hover),
  :global(.export-trigger[data-state="open"]) {
    color: var(--ink);
    border-color: var(--accent);
  }
</style>
