<script lang="ts">
  // The behavioural-spec lens: a `feature` scenario's given/when/then steps drawn
  // as a connected flow (LANG.md §9.5). All positioning comes from
  // `pseudoscript-emit::layout_feature_scene`; this component is a dumb renderer
  // of the positioned scene, a single full-canvas overlay node so Svelte Flow
  // provides pan / zoom / minimap / fit-to-view.
  import { Background, Controls, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import type { Edge, Node } from "@xyflow/svelte";
  import FeatureSteps from "./FeatureSteps.svelte";
  import DiagramExport from "./DiagramExport.svelte";
  import { theme } from "$lib/theme.svelte.js";
  // The positioned feature scene from `layout_feature_scene` (steps carry rects).
  import type { FeatureLayout as Layout } from "$lib/core/diagram-scene.js";

  type Props = { scene: { entry?: string | null }; layout: Layout | null };
  let { scene, layout }: Props = $props();

  const colorMode = $derived(theme.resolved === "light" ? "light" : "dark");
  const nodeTypes = { steps: FeatureSteps };

  let flowEl = $state<HTMLDivElement | null>(null);
  const exportName = $derived((scene?.entry ?? "").split("::").pop() || "feature");

  function build(l: Layout | null): { nodes: Node[]; edges: Edge[] } {
    if (!l || !Array.isArray(l.steps)) return { nodes: [], edges: [] };
    return {
      nodes: [
        {
          id: "__steps",
          type: "steps",
          position: { x: 0, y: 0 },
          width: l.width,
          height: l.height,
          data: { name: l.name, targetLabel: l.target_label, steps: l.steps, width: l.width, height: l.height },
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
