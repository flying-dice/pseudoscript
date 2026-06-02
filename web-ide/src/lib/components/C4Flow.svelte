<script lang="ts">
  // Interactive C4 graph: structure at a glance. Node geometry comes from the
  // chosen layout algorithm (layered / grid / circular / radial); Svelte Flow
  // provides pan / zoom / minimap / fit-to-view. Nodes are immutable — not
  // draggable or connectable — so the diagram stays a true projection of the
  // model. Edges float: they anchor at the nearest card borders (shortest path)
  // rather than fixed handles. A boundary view (container / component) draws its
  // `of` node as an enclosing box rather than a peer card. A floating "Customise"
  // button opens the layout/edge modal; right-clicking a node opens its context
  // menu (drill / flows / go-to-definition / find-usages).
  import { Background, Controls, MarkerType, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import type { Edge, Node } from "@xyflow/svelte";
  import Dagre from "@dagrejs/dagre";
  import type { Graph } from "@dagrejs/dagre";
  import BoundaryNode from "./BoundaryNode.svelte";
  import C4Node from "./C4Node.svelte";
  import CanvasSettings from "./CanvasSettings.svelte";
  import DiagramExport from "./DiagramExport.svelte";
  import FitView from "./FitView.svelte";
  import FloatingEdge from "./FloatingEdge.svelte";
  import { theme } from "$lib/theme.svelte.js";
  import { canvasPrefs } from "$lib/stores/canvas-prefs.svelte.js";
  import type { LayoutAlgo, LayoutDir } from "$lib/stores/canvas-prefs.svelte.js";

  // A node in the projected scene (one C4 element).
  type SceneNode = {
    fqn: string;
    label: string;
    kind: string;
    summary?: string;
    boundary?: string | null;
  };
  // A relationship between two scene nodes.
  type SceneEdge = {
    from: string;
    to: string;
    label?: string;
    kind: string;
  };
  // The structural scene this component projects: nodes, their relationships,
  // and (for a boundary view) the `of` element drawn as the enclosing box.
  type Scene = {
    nodes: SceneNode[];
    edges: SceneEdge[];
    of?: string | null;
  };
  // The `data` payload carried by every Svelte Flow node (card or boundary).
  type NodeData = {
    label: string;
    kind: string;
    summary: string;
    fqn: string;
    boundary?: boolean;
    onclose?: () => void;
  };
  // An entry-point flow offered in a node's context menu.
  type Flow = { fqn: string; name: string; triggered?: boolean };

  type Props = {
    scene: Scene;
    onpick?: ((fqn: string) => void) | null;
    onup?: (() => void) | null;
    flows?: Map<string, Flow[]> | null;
    onsource?: ((fqn: string) => void) | null;
    onusages?: ((fqn: string, event: MouseEvent) => void) | null;
  };

  let { scene, onpick, onup, flows = null, onsource = null, onusages = null }: Props = $props();

  // Drive Svelte Flow's colour mode from the app theme so the canvas (pane,
  // grid, minimap, controls) follows light/dark instead of being pinned dark.
  const colorMode = $derived(theme.resolved === "light" ? "light" : "dark");

  // The canvas root, captured for diagram export.
  let flowEl = $state<HTMLDivElement | null>(null);
  // Download name: the boundary view's subject, else a generic fallback.
  const exportName = $derived((scene.of ?? "").split("::").pop() || "diagram");

  const nodeTypes = { boundary: BoundaryNode, card: C4Node };
  const edgeTypes = { floating: FloatingEdge };

  const NODE_W = 200;
  const NODE_H = 104;
  const PAD = 34; // inner gap between the boundary box and its children
  const TITLE_H = 30; // room at the top of the box for the boundary's label

  // Which deeper view a node drills into, by kind. Persons / components have no
  // structural view below them, so they get no drill button (info only).
  const DRILL: Record<string, string> = { system: "Open container diagram", container: "Open component diagram" };

  function dagreGraph(rankdir: LayoutDir): Graph {
    const g = new Dagre.graphlib.Graph();
    g.setGraph({ rankdir, nodesep: 60, ranksep: 90, marginx: 28, marginy: 28 });
    g.setDefaultEdgeLabel(() => ({}));
    return g;
  }

  function card(n: SceneNode, parentId?: string) {
    return {
      id: n.fqn,
      type: "card",
      data: { label: n.label, kind: n.kind, summary: n.summary ?? "", fqn: n.fqn } as NodeData,
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
  function grouped(s: Scene, boundaryFqn: string, rankdir: LayoutDir): Node[] {
    const anchor = s.nodes.find((n) => n.fqn === boundaryFqn);
    const inside = s.nodes.filter((n) => n.boundary === boundaryFqn);
    const outside = s.nodes.filter((n) => n.fqn !== boundaryFqn && n.boundary !== boundaryFqn);

    // Inner pass: children, laid out by the edges among them.
    const inner = dagreGraph(rankdir);
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
    const outer = dagreGraph(rankdir);
    outer.setNode(boundaryFqn, { width: boxW, height: boxH });
    for (const n of outside) outer.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    const lift = (fqn: string): string => (insideSet.has(fqn) ? boundaryFqn : fqn);
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
      data: { label: anchor?.label ?? boundaryFqn, kind: anchor?.kind ?? "system", summary: anchor?.summary ?? "", fqn: boundaryFqn, boundary: true, onclose: onup ?? undefined } as NodeData,
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
    return [boundaryNode, ...childNodes, ...outsideNodes] as Node[];
  }

  // Lay out positions over a single index, given a position function. Shared by
  // the geometric algorithms (grid / circular / radial); origin is irrelevant —
  // fitView re-centres — so they place cards in a tidy local frame.
  const COL_GAP = 80;
  const ROW_GAP = 64;

  function placed(s: Scene, pos: (i: number, n: SceneNode) => { x: number; y: number }): Node[] {
    return s.nodes.map((n, i) => ({ ...card(n), position: pos(i, n) })) as Node[];
  }

  // Hierarchical: dagre layered, honouring the flow direction.
  function layered(s: Scene, rankdir: LayoutDir): Node[] {
    const g = dagreGraph(rankdir);
    for (const n of s.nodes) g.setNode(n.fqn, { width: NODE_W, height: NODE_H });
    for (const e of s.edges) g.setEdge(e.from, e.to);
    Dagre.layout(g);
    return s.nodes.map((n) => {
      const p = g.node(n.fqn);
      return { ...card(n), position: { x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 } };
    }) as Node[];
  }

  // Top-left of the k-th of `count` cards spaced evenly on a circle of `radius`,
  // centred on the origin.
  function onCircle(radius: number, k: number, count: number): { x: number; y: number } {
    const a = (2 * Math.PI * k) / count - Math.PI / 2;
    return { x: radius * Math.cos(a) - NODE_W / 2, y: radius * Math.sin(a) - NODE_H / 2 };
  }

  // Radius that seats `count` cards around a ring without overlap.
  function ringRadius(count: number): number {
    return Math.max(NODE_W, ((NODE_W + COL_GAP) * count) / (2 * Math.PI));
  }

  // Grid: row-major into the squarest grid that holds every card.
  function grid(s: Scene): Node[] {
    const cols = Math.max(1, Math.ceil(Math.sqrt(s.nodes.length)));
    return placed(s, (i) => ({
      x: (i % cols) * (NODE_W + COL_GAP),
      y: Math.floor(i / cols) * (NODE_H + ROW_GAP),
    }));
  }

  // Circular: cards spaced evenly around one ring, sized so they don't overlap.
  function circular(s: Scene): Node[] {
    const count = s.nodes.length;
    const r = ringRadius(count);
    return placed(s, (i) => onCircle(r, i, count));
  }

  // Ring index (hop distance from `roots`) of every id; anything unreached lands
  // one ring beyond the deepest reached.
  function hopRings(ids: string[], out: Map<string, string[]>, roots: string[]): Map<string, number> {
    const ring = new Map<string, number>();
    let frontier = roots;
    let depth = 0;
    while (frontier.length) {
      const next: string[] = [];
      for (const id of frontier) {
        if (ring.has(id)) continue;
        ring.set(id, depth);
        for (const m of out.get(id) ?? []) if (!ring.has(m)) next.push(m);
      }
      frontier = next;
      depth++;
    }
    const maxRing = ring.size ? Math.max(...ring.values()) : 0;
    for (const id of ids) if (!ring.has(id)) ring.set(id, maxRing + 1);
    return ring;
  }

  // Radial: concentric rings by hop distance from the roots (sources with no
  // incoming edge), so an entry point sits at the centre and dependents fan out.
  function radial(s: Scene): Node[] {
    const ids = s.nodes.map((n) => n.fqn);
    const out = new Map<string, string[]>(ids.map((id) => [id, []]));
    const indeg = new Map<string, number>(ids.map((id) => [id, 0]));
    for (const e of s.edges) {
      if (out.has(e.from) && indeg.has(e.to)) {
        out.get(e.from)!.push(e.to);
        indeg.set(e.to, (indeg.get(e.to) ?? 0) + 1);
      }
    }
    let roots = ids.filter((id) => (indeg.get(id) ?? 0) === 0);
    if (roots.length === 0) roots = ids.slice(0, 1);

    const ring = hopRings(ids, out, roots);

    // Group ids by ring; give each ring a radius that seats its members and clears
    // the ring inside it (a lone centre sits at 0), then place members around it.
    const byRing = new Map<number, string[]>();
    for (const id of ids) {
      const d = ring.get(id) ?? 0;
      if (!byRing.has(d)) byRing.set(d, []);
      byRing.get(d)!.push(id);
    }
    const RING_STEP = NODE_W + ROW_GAP + 40;
    const place = new Map<string, { x: number; y: number }>();
    let prev = 0;
    for (const d of [...byRing.keys()].sort((a, b) => a - b)) {
      const members = byRing.get(d)!;
      const radius = d === 0 && members.length === 1 ? 0 : Math.max(prev + RING_STEP, ringRadius(members.length));
      prev = radius;
      members.forEach((id, k) => place.set(id, onCircle(radius, k, members.length)));
    }
    return placed(s, (_i, n) => place.get(n.fqn) ?? { x: 0, y: 0 });
  }

  // A flat view (context, or a boundary with no children): every node a peer card,
  // placed by the chosen algorithm. Direction applies to the layered algorithm only.
  function flat(s: Scene, algo: LayoutAlgo, rankdir: LayoutDir): Node[] {
    switch (algo) {
      case "grid":
        return grid(s);
      case "circular":
        return circular(s);
      case "radial":
        return radial(s);
      default:
        return layered(s, rankdir);
    }
  }

  function layout(s: Scene, algo: LayoutAlgo, rankdir: LayoutDir, edgeType: string): { nodes: Node[]; edges: Edge[] } {
    const boundaryFqn = s.of ?? null;
    const hasChildren = boundaryFqn && s.nodes.some((n) => n.boundary === boundaryFqn);
    // A boundary view is a nested-box layout — always layered; the algorithm
    // choice applies to the flat (peer) views.
    const nodes = hasChildren ? grouped(s, boundaryFqn, rankdir) : flat(s, algo, rankdir);
    const edges = s.edges.map((e, i) => ({
      id: `e${i}`,
      source: e.from,
      target: e.to,
      label: e.label || undefined,
      // Floating: anchored at the nearest borders, routed in the chosen style.
      type: "floating",
      data: { pathType: edgeType },
      animated: true,
      class: `c4-edge ${e.kind}`,
      selectable: false,
      markerEnd: {
        type: MarkerType.ArrowClosed,
        width: 14,
        height: 14,
        color: e.kind === "trigger" ? "var(--k-callable)" : "var(--line-strong)",
      },
    })) as Edge[];
    return { nodes, edges };
  }

  // Re-project whenever the scene or the algorithm / direction / edge preferences
  // change. The graph is a projection, so a wholesale re-layout is the right call.
  const initial = layout(scene, canvasPrefs.algorithm, canvasPrefs.layout, canvasPrefs.edgeType);
  let nodes = $state<Node[]>(initial.nodes);
  let edges = $state<Edge[]>(initial.edges);
  $effect(() => {
    const l = layout(scene, canvasPrefs.algorithm, canvasPrefs.layout, canvasPrefs.edgeType);
    nodes = l.nodes;
    edges = l.edges;
  });

  // The node a right-click opened the context menu on, anchored at the pointer.
  // `event` is kept so "Find usages" can position its popover where the click was.
  type MenuState = { fqn: string; kind: string; label: string; isBoundary: boolean; x: number; y: number; event: MouseEvent };
  let menu = $state<MenuState | null>(null);
  const menuFlows = $derived<Flow[]>(menu && flows ? (flows.get(menu.fqn) ?? []) : []);
  let menuEl = $state<HTMLDivElement | null>(null);

  function onnodecontextmenu({ event, node }: { event: MouseEvent; node: Node }): void {
    event.preventDefault();
    const data = node.data as NodeData;
    menu = { fqn: data.fqn, kind: data.kind, label: data.label, isBoundary: node.type === "boundary", x: event.clientX, y: event.clientY, event };
  }
  const closeMenu = () => (menu = null);
  // Run a menu action and dismiss.
  function act(run: () => void): void {
    run();
    closeMenu();
  }

  // Render the menu under <body> so its `position: fixed` resolves against the
  // viewport. Inside the canvas islands a transformed/animated ancestor forms a
  // containing block, which would otherwise offset `left: clientX` by the rail +
  // explorer width.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  // Move keyboard focus to the menu when it opens so arrows / Enter / Escape work.
  $effect(() => {
    if (menu) menuEl?.focus();
  });
</script>

<svelte:window onkeydown={(e) => menu && e.key === "Escape" && closeMenu()} />

<div class="flow" bind:this={flowEl}>
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    {edgeTypes}
    fitView
    {colorMode}
    minZoom={0.2}
    maxZoom={2.5}
    nodesDraggable={false}
    nodesConnectable={false}
    elementsSelectable={false}
    proOptions={{ hideAttribution: true }}
    {onnodecontextmenu}
  >
    <Background gap={24} />
    <MiniMap pannable zoomable />
    <Controls showLock={false} />
    <!-- Re-frame the viewport when the algorithm / direction moves the nodes.
         The flow stays mounted, so nodes keep their measured sizes and fitView
         frames them correctly (a remount would refit before re-measuring). -->
    <FitView sig={`${canvasPrefs.algorithm}|${canvasPrefs.layout}`} />
  </SvelteFlow>

  <!-- Top-right toolbar: export the diagram, and the layout/edge modal. -->
  <div class="customise">
    <DiagramExport container={flowEl} {nodes} filename={exportName} />
    <CanvasSettings />
  </div>

  {#if menu}
    {@const m = menu}
    <div use:portal>
    <!-- A transparent layer that dismisses on any click or another right-click. -->
    <button
      class="menu-scrim"
      aria-label="Close menu"
      onclick={closeMenu}
      oncontextmenu={(e) => {
        e.preventDefault();
        closeMenu();
      }}
    ></button>
    <div bind:this={menuEl} class="ctx-menu" role="menu" tabindex="-1" aria-label="Node actions" style="left:{m.x}px; top:{m.y}px">
      <div class="ctx-head">
        <span class="kind {m.kind}">{m.kind}</span>
        <span class="ctx-name">{m.label}</span>
      </div>
      <div class="ctx-sep"></div>

      {#if m.isBoundary}
        {#if onup}
          <button role="menuitem" class="ctx-item" onclick={() => act(() => onup?.())}>Go up a level</button>
        {/if}
      {:else if DRILL[m.kind]}
        <button role="menuitem" class="ctx-item" onclick={() => act(() => onpick?.(m.fqn))}>{DRILL[m.kind]}</button>
      {/if}

      {#if menuFlows.length}
        <div class="ctx-label">Flows</div>
        {#each menuFlows as f (f.fqn)}
          <button role="menuitem" class="ctx-item flow" onclick={() => act(() => onpick?.(f.fqn))}>
            <span class="play">▶</span><span class="flow-name">{f.name}</span>{#if f.triggered}<span class="trig">triggered</span>{/if}
          </button>
        {/each}
      {/if}

      <div class="ctx-sep"></div>
      <button role="menuitem" class="ctx-item" onclick={() => act(() => onsource?.(m.fqn))}>Go to definition</button>
      <button role="menuitem" class="ctx-item" onclick={() => act(() => onusages?.(m.fqn, m.event))}>Find usages</button>
    </div>
    </div>
  {/if}
</div>

<style>
  .flow { position: relative; width: 100%; height: 100%; }

  /* Floating "Customise" button, top-right, clear of the minimap (bottom-right). */
  .customise {
    position: absolute;
    top: 0.7rem;
    right: 0.7rem;
    z-index: 5;
    display: flex;
    gap: 0.4rem;
  }

  .menu-scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }
  .ctx-menu {
    position: fixed;
    z-index: 61;
    min-width: 13rem;
    max-width: 18rem;
    padding: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    outline: none;
  }
  .ctx-head {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.3rem 0.45rem 0.4rem;
  }
  .ctx-head .kind {
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    border-left: 2px solid var(--k, var(--ink-faint));
    background: var(--surface-3);
  }
  .ctx-head .kind.person { --k: var(--k-person); }
  .ctx-head .kind.system { --k: var(--k-system); }
  .ctx-head .kind.container { --k: var(--k-container); }
  .ctx-head .kind.component { --k: var(--k-component); }
  .ctx-head .kind.data { --k: var(--k-data); }
  .ctx-head .kind.callable { --k: var(--k-callable); }
  .ctx-name {
    font-family: var(--font-display);
    font-size: 0.86rem;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx-sep {
    height: 1px;
    margin: 0.2rem 0.2rem;
    background: var(--line);
  }
  .ctx-label {
    padding: 0.3rem 0.5rem 0.15rem;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .ctx-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.4rem 0.5rem;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    color: var(--ink-soft);
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
  }
  .ctx-item:hover,
  .ctx-item:focus-visible {
    background: var(--surface-2);
    color: var(--ink);
    outline: none;
  }
  .ctx-item .flow-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx-item .play {
    flex: none;
    color: var(--k-callable);
    font-size: 0.58rem;
  }
  .ctx-item .trig {
    margin-left: auto;
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }
</style>
