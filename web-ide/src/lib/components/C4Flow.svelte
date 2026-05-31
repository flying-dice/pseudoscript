<script>
  // Interactive C4 graph: structure at a glance. Node geometry comes from dagre
  // auto-layout; Svelte Flow provides pan / zoom / minimap / fit-to-view. Nodes
  // are immutable — not draggable or connectable — so the diagram stays a true
  // projection of the model. A boundary view (container / component) draws its
  // `of` node as an enclosing box rather than a peer card. Clicking a node opens
  // an info popover; drilling in is an explicit button there, not the click.
  import { Background, Controls, MarkerType, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import Dagre from "@dagrejs/dagre";
  import BoundaryNode from "./BoundaryNode.svelte";
  import C4Node from "./C4Node.svelte";

  let { scene, onpick, onup, flows = null } = $props();

  const nodeTypes = { boundary: BoundaryNode, card: C4Node };

  const NODE_W = 200;
  const NODE_H = 104;
  const PAD = 34; // inner gap between the boundary box and its children
  const TITLE_H = 30; // room at the top of the box for the boundary's label

  // Which deeper view a node drills into, by kind. Persons / components have no
  // structural view below them, so they get no drill button (info only).
  const DRILL = { system: "Open container diagram", container: "Open component diagram" };

  function dagreGraph() {
    const g = new Dagre.graphlib.Graph();
    g.setGraph({ rankdir: "TB", nodesep: 60, ranksep: 90, marginx: 28, marginy: 28 });
    g.setDefaultEdgeLabel(() => ({}));
    return g;
  }

  function card(n, parentId) {
    return {
      id: n.fqn,
      type: "card",
      data: { label: n.label, kind: n.kind, summary: n.summary ?? "", fqn: n.fqn },
      class: `c4-node ${n.kind}`,
      width: NODE_W,
      height: NODE_H,
      draggable: false,
      connectable: false,
      ...(parentId ? { parentId } : {}),
    };
  }

  // A boundary view: lay the `of` children inside a box, the external actors
  // around it. Two dagre passes — one inside the box, one placing the box and
  // its outside actors — keep the box from overlapping anything.
  function grouped(s, boundaryFqn) {
    const anchor = s.nodes.find((n) => n.fqn === boundaryFqn);
    const inside = s.nodes.filter((n) => n.boundary === boundaryFqn);
    const outside = s.nodes.filter((n) => n.fqn !== boundaryFqn && n.boundary !== boundaryFqn);

    // Inner pass: children, laid out by the edges among them.
    const inner = dagreGraph();
    for (const n of inside) inner.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    const insideSet = new Set(inside.map((n) => n.fqn));
    for (const e of s.edges) if (insideSet.has(e.from) && insideSet.has(e.to)) inner.setEdge(e.from, e.to);
    Dagre.layout(inner);

    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const n of inside) {
      const p = inner.node(n.fqn);
      minX = Math.min(minX, p.x - NODE_W / 2);
      minY = Math.min(minY, p.y - NODE_H / 2);
      maxX = Math.max(maxX, p.x + NODE_W / 2);
      maxY = Math.max(maxY, p.y + NODE_H / 2);
    }
    const boxW = maxX - minX + PAD * 2;
    const boxH = maxY - minY + PAD * 2 + TITLE_H;

    // Outer pass: the box (as one node) plus the external actors, positioned by
    // the edges that cross the boundary.
    const outer = dagreGraph();
    outer.setNode(boundaryFqn, { width: boxW, height: boxH });
    for (const n of outside) outer.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    const lift = (fqn) => (insideSet.has(fqn) ? boundaryFqn : fqn);
    for (const e of s.edges) {
      const from = lift(e.from), to = lift(e.to);
      if (from !== to && outer.hasNode(from) && outer.hasNode(to)) outer.setEdge(from, to);
    }
    Dagre.layout(outer);

    const boxPos = outer.node(boundaryFqn);
    const boxOrigin = { x: boxPos.x - boxW / 2, y: boxPos.y - boxH / 2 };

    // The box first (Svelte Flow needs a parent before its children), then the
    // framed children positioned relative to it, then the outside actors.
    const boundaryNode = {
      id: boundaryFqn,
      type: "boundary",
      position: boxOrigin,
      data: { label: anchor?.label ?? boundaryFqn, kind: anchor?.kind ?? "system", summary: anchor?.summary ?? "", fqn: boundaryFqn, boundary: true, onclose: onup },
      class: `c4-boundary ${anchor?.kind ?? "system"}`,
      width: boxW,
      height: boxH,
      draggable: false,
      connectable: false,
      selectable: true,
    };
    const childNodes = inside.map((n) => {
      const p = inner.node(n.fqn);
      return {
        ...card(n, boundaryFqn),
        position: { x: p.x - NODE_W / 2 - minX + PAD, y: p.y - NODE_H / 2 - minY + PAD + TITLE_H },
      };
    });
    const outsideNodes = outside.map((n) => {
      const p = outer.node(n.fqn);
      return { ...card(n), position: { x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 } };
    });
    return [boundaryNode, ...childNodes, ...outsideNodes];
  }

  // A flat view (context, or a boundary with no children): every node a peer card.
  function flat(s) {
    const g = dagreGraph();
    for (const n of s.nodes) g.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    for (const e of s.edges) g.setEdge(e.from, e.to);
    Dagre.layout(g);
    return s.nodes.map((n) => {
      const p = g.node(n.fqn);
      return { ...card(n), position: { x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 } };
    });
  }

  function layout(s) {
    const boundaryFqn = s.of ?? null;
    const hasChildren = boundaryFqn && s.nodes.some((n) => n.boundary === boundaryFqn);
    const nodes = hasChildren ? grouped(s, boundaryFqn) : flat(s);
    const edges = s.edges.map((e, i) => ({
      id: `e${i}`,
      source: e.from,
      target: e.to,
      label: e.label || undefined,
      type: "smoothstep",
      animated: true,
      class: `c4-edge ${e.kind}`,
      selectable: false,
      markerEnd: {
        type: MarkerType.ArrowClosed,
        width: 14,
        height: 14,
        color: e.kind === "trigger" ? "var(--k-callable)" : "var(--line-strong)",
      },
    }));
    return { nodes, edges };
  }

  const initial = layout(scene);
  let nodes = $state(initial.nodes);
  let edges = $state(initial.edges);

  // Clicking a node opens its info popover; drilling in (a deeper structural
  // view, or a behavioural flow) is an explicit action there.
  let picked = $state(null);
  const pickedFlows = $derived(picked && flows ? (flows.get(picked.fqn) ?? []) : []);
  function onnodeclick({ node }) {
    picked = node.data;
  }
  function open(fqn) {
    onpick?.(fqn);
    picked = null;
  }
</script>

<div class="flow">
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    fitView
    colorMode="dark"
    minZoom={0.2}
    maxZoom={2.5}
    nodesDraggable={false}
    nodesConnectable={false}
    proOptions={{ hideAttribution: true }}
    {onnodeclick}
  >
    <Background gap={24} />
    <MiniMap pannable zoomable />
    <Controls showLock={false} />
  </SvelteFlow>

  {#if picked}
    <button class="scrim" aria-label="Close" onclick={() => (picked = null)}></button>
    <div class="popover" role="dialog" aria-modal="true">
      <span class="kind {picked.kind}">{picked.kind}</span>
      <h3>{picked.label}</h3>
      <p class="fqn">{picked.fqn}</p>
      {#if picked.summary}<p class="summary">{picked.summary}</p>{/if}
      {#if pickedFlows.length}
        <div class="flows">
          <span class="flows-label">Flows</span>
          {#each pickedFlows as f (f.fqn)}
            <button class="flow" onclick={() => open(f.fqn)}>
              <span class="play">▶</span>{f.name}{#if f.triggered}<span class="trig">triggered</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
      <div class="actions">
        {#if DRILL[picked.kind]}
          <button class="drill" onclick={() => open(picked.fqn)}>{DRILL[picked.kind]} →</button>
        {/if}
        <button class="dismiss" onclick={() => (picked = null)}>Close</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .flow { position: relative; width: 100%; height: 100%; }

  .scrim {
    position: absolute;
    inset: 0;
    z-index: 8;
    border: 0;
    padding: 0;
    background: color-mix(in srgb, var(--bg) 35%, transparent);
    cursor: default;
  }
  .popover {
    position: absolute;
    z-index: 9;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(22rem, calc(100% - 2rem));
    padding: 1.1rem 1.2rem 1rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    box-shadow: 0 18px 48px -18px rgba(0, 0, 0, 0.85);
  }
  .popover .kind {
    display: inline-block;
    font-family: var(--font-mono);
    font-size: 0.56rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    padding: 0.12rem 0.4rem;
    border-radius: 4px;
    border-left: 2px solid var(--k, var(--ink-faint));
    background: var(--surface-3);
  }
  .popover .kind.person { --k: var(--k-person); }
  .popover .kind.system { --k: var(--k-system); }
  .popover .kind.container { --k: var(--k-container); }
  .popover .kind.component { --k: var(--k-component); }
  .popover .kind.data { --k: var(--k-data); }
  .popover .kind.callable { --k: var(--k-callable); }
  .popover h3 {
    margin: 0.55rem 0 0.2rem;
    font-family: var(--font-display);
    font-size: 1.05rem;
    color: var(--ink);
  }
  .popover .fqn { margin: 0; font-family: var(--font-mono); font-size: 0.72rem; color: var(--ink-faint); }
  .popover .summary { margin: 0.7rem 0 0; font-size: 0.85rem; line-height: 1.6; color: var(--ink-soft); }
  .popover .flows { display: flex; flex-direction: column; gap: 0.3rem; margin-top: 0.9rem; }
  .popover .flows-label {
    font-family: var(--font-mono);
    font-size: 0.56rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    margin-bottom: 0.15rem;
  }
  .popover .flow {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.4rem 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--ink);
    background: var(--surface-3);
    border: 1px solid var(--line);
    border-left: 2px solid var(--k-callable);
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
  }
  .popover .flow:hover { border-color: var(--accent); }
  .popover .flow .play { color: var(--k-callable); font-size: 0.6rem; }
  .popover .flow .trig {
    margin-left: auto;
    font-size: 0.56rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .popover .actions { display: flex; gap: 0.5rem; margin-top: 1.1rem; }
  .popover .drill {
    flex: 1;
    padding: 0.5rem 0.8rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--accent-ink);
    background: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 6px;
    cursor: pointer;
  }
  .popover .drill:hover { background: var(--accent-hi); }
  .popover .dismiss {
    padding: 0.5rem 0.8rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--ink-soft);
    background: var(--surface-3);
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    cursor: pointer;
  }
  .popover .dismiss:hover { color: var(--ink); border-color: var(--accent); }
</style>
