<script>
  // Client-only: an interactive C4 graph. Node geometry comes from dagre
  // auto-layout in the browser; Svelte Flow provides pan/zoom/minimap/fit.
  import { SvelteFlow, Background, Controls, MiniMap } from "@xyflow/svelte";
  import Dagre from "@dagrejs/dagre";

  let { scene } = $props();

  const NODE_W = 188;
  const NODE_H = 66;

  function layout(scene) {
    const g = new Dagre.graphlib.Graph();
    g.setGraph({ rankdir: "TB", nodesep: 56, ranksep: 84, marginx: 24, marginy: 24 });
    g.setDefaultEdgeLabel(() => ({}));
    for (const n of scene.nodes) g.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    for (const e of scene.edges) g.setEdge(e.from, e.to);
    Dagre.layout(g);

    const nodes = scene.nodes.map((n) => {
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
    const edges = scene.edges.map((e, i) => ({
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
    <Background gap={22} />
    <MiniMap pannable zoomable />
    <Controls showLock={false} />
  </SvelteFlow>
</div>
