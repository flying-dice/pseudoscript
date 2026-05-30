<script>
  // Interactive C4 graph: structure at a glance. Node geometry comes from dagre
  // auto-layout; Svelte Flow provides pan / zoom / minimap / fit-to-view.
  import { Background, Controls, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import Dagre from "@dagrejs/dagre";
  import "@xyflow/svelte/dist/style.css";

  let { scene } = $props();

  const NODE_W = 196;
  const NODE_H = 70;

  function layout(s) {
    const g = new Dagre.graphlib.Graph();
    g.setGraph({ rankdir: "TB", nodesep: 60, ranksep: 90, marginx: 28, marginy: 28 });
    g.setDefaultEdgeLabel(() => ({}));
    for (const n of s.nodes) g.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    for (const e of s.edges) g.setEdge(e.from, e.to);
    Dagre.layout(g);

    const nodes = s.nodes.map((n) => {
      const p = g.node(n.fqn);
      return {
        id: n.fqn,
        position: { x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 },
        data: { label: n.label, kind: n.kind, summary: n.summary ?? "" },
        class: `c4-node ${n.kind}`,
        width: NODE_W,
        height: NODE_H,
      };
    });
    const edges = s.edges.map((e, i) => ({
      id: `e${i}`,
      source: e.from,
      target: e.to,
      label: e.label || undefined,
      type: "smoothstep",
      animated: e.kind === "trigger",
      class: `c4-edge ${e.kind}`,
    }));
    return { nodes, edges };
  }

  const initial = layout(scene);
  let nodes = $state(initial.nodes);
  let edges = $state(initial.edges);
</script>

<div class="flow">
  <SvelteFlow bind:nodes bind:edges fitView minZoom={0.2} maxZoom={2.5} proOptions={{ hideAttribution: true }}>
    <Background gap={24} />
    <MiniMap pannable zoomable />
    <Controls showLock={false} />
  </SvelteFlow>
</div>

<style>
  .flow { width: 100%; height: 100%; }
</style>
