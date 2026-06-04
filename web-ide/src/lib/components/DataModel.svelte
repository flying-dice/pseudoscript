<script lang="ts">
  // The structural lens: a `data` type's entity (ER) view — its fields and the
  // data types they reference, one hop out (LANG.md §9.4). All positioning comes
  // from `pseudoscript-emit::layout_data_scene`; this component is a dumb renderer
  // of the positioned scene, drawn as a single full-canvas overlay node so Svelte
  // Flow provides pan / zoom / minimap / fit-to-view. Clicking a card navigates
  // to that type.
  import { Background, Controls, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import type { Edge, Node } from "@xyflow/svelte";
  import DataEntities from "./DataEntities.svelte";
  import DiagramExport from "./DiagramExport.svelte";
  import { theme } from "$lib/theme.svelte.js";
  // The positioned data scene from `layout_data_scene` (entities carry rects).
  import type { DataLayout as Layout } from "$lib/core/diagram-scene.js";

  type Props = {
    scene: { of?: string | null };
    layout: Layout | null;
    onpick?: ((fqn: string) => void) | null;
  };
  let { scene, layout, onpick = null }: Props = $props();

  const colorMode = $derived(theme.resolved === "light" ? "light" : "dark");
  const nodeTypes = { entities: DataEntities };

  let flowEl = $state<HTMLDivElement | null>(null);
  const exportName = $derived((scene?.of ?? "").split("::").pop() || "data");

  function build(l: Layout | null): { nodes: Node[]; edges: Edge[] } {
    if (!l || !Array.isArray(l.entities)) return { nodes: [], edges: [] };
    return {
      nodes: [
        {
          id: "__entities",
          type: "entities",
          position: { x: 0, y: 0 },
          width: l.width,
          height: l.height,
          data: { entities: l.entities, links: l.links, width: l.width, height: l.height, onpick },
          draggable: false,
          selectable: false,
          connectable: false,
        },
      ],
      edges: [],
    };
  }

  const built = $derived(build(layout));
  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
  $effect(() => {
    nodes = built.nodes;
    edges = built.edges;
  });
</script>

<div class="flow" bind:this={flowEl}>
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    fitView
    {colorMode}
    minZoom={0.2}
    maxZoom={2.5}
    nodesDraggable={false}
    nodesConnectable={false}
    elementsSelectable={false}
    proOptions={{ hideAttribution: true }}
  >
    <Background gap={24} />
    <MiniMap pannable zoomable />
    <Controls showLock={false} />
  </SvelteFlow>

  <div class="customise">
    <DiagramExport container={flowEl} {nodes} filename={exportName} />
  </div>
</div>

<style>
  .flow {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .customise {
    position: absolute;
    top: 0.7rem;
    right: 0.7rem;
    z-index: 5;
    display: flex;
    gap: 0.4rem;
  }
</style>
