<script lang="ts">
  // Interactive C4 graph: structure at a glance. All geometry comes from the
  // `pseudoscript-emit` layout engine (the same layout-rs Sugiyama pass the
  // static SVG draws), handed in as a positioned `C4Layout`; this component is a
  // dumb renderer of it — it computes no layout. Svelte Flow provides pan / zoom
  // / minimap / fit-to-view. Nodes are immutable (not draggable or connectable),
  // so the diagram stays a true projection. Edges follow the engine's routed
  // polylines. A boundary view (container / component) draws its `of` node as an
  // enclosing frame. Right-clicking a node opens its context menu (drill / flows
  // / go-to-definition / find-usages).
  import { Background, Controls, MarkerType, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import type { Edge, Node } from "@xyflow/svelte";
  import BoundaryNode from "./BoundaryNode.svelte";
  import C4Node from "./C4Node.svelte";
  import CanvasMenu from "./CanvasMenu.svelte";
  import DiagramExport from "./DiagramExport.svelte";
  import PolylineEdge from "./PolylineEdge.svelte";
  import { theme } from "$lib/theme.svelte.js";
  import type { MenuItem, MenuSection } from "$lib/core/types.js";

  // A scene only contributes its boundary subject here (the export name); all
  // geometry comes from the layout.
  type Scene = { of?: string | null };
  // The positioned C4 layout produced by `pseudoscript-emit::layout_c4_scene`.
  type Rect = { x: number; y: number; w: number; h: number };
  type Pt = { x: number; y: number };
  type LaidOutNode = { fqn: string; kind: string; label: string; summary?: string | null; rect: Rect };
  type LaidOutEdge = {
    from: string;
    to: string;
    kind: string;
    label: string;
    points: Pt[];
    label_pos?: Pt | null;
    dashed: boolean;
  };
  type Layout = {
    width: number;
    height: number;
    nodes: LaidOutNode[];
    edges: LaidOutEdge[];
    boundary?: { title: string; kind: string; rect: Rect } | null;
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
    layout: Layout | null;
    onpick?: ((fqn: string) => void) | null;
    onup?: (() => void) | null;
    flows?: Map<string, Flow[]> | null;
    onsource?: ((fqn: string) => void) | null;
    onusages?: ((fqn: string, event: MouseEvent) => void) | null;
  };

  let { scene, layout, onpick, onup, flows = null, onsource = null, onusages = null }: Props = $props();

  // Drive Svelte Flow's colour mode from the app theme so the canvas (pane,
  // grid, minimap, controls) follows light/dark instead of being pinned dark.
  const colorMode = $derived(theme.resolved === "light" ? "light" : "dark");

  // The canvas root, captured for diagram export.
  let flowEl = $state<HTMLDivElement | null>(null);
  // Download name: the boundary view's subject, else a generic fallback.
  const exportName = $derived((scene.of ?? "").split("::").pop() || "diagram");

  const nodeTypes = { boundary: BoundaryNode, card: C4Node };
  const edgeTypes = { polyline: PolylineEdge };

  // Which deeper view a node drills into, by kind. Persons / components have no
  // structural view below them, so they get no drill button (info only).
  const DRILL: Record<string, string> = { system: "Open container diagram", container: "Open component diagram" };

  // Map the positioned layout into Svelte Flow nodes + edges. The boundary frame
  // sits behind (zIndex 0) as a non-interactive box; cards sit on top at their
  // engine-computed rect (position + size); edges follow the routed polylines.
  function build(l: Layout | null): { nodes: Node[]; edges: Edge[] } {
    if (!l || !Array.isArray(l.nodes)) return { nodes: [], edges: [] };

    const frame: Node[] = l.boundary
      ? [
          {
            id: "__boundary",
            type: "boundary",
            position: { x: l.boundary.rect.x, y: l.boundary.rect.y },
            width: l.boundary.rect.w,
            height: l.boundary.rect.h,
            data: { label: l.boundary.title, kind: l.boundary.kind, summary: "", fqn: scene.of ?? "", boundary: true, onclose: onup ?? undefined } as NodeData,
            class: `c4-boundary ${l.boundary.kind}`,
            draggable: false,
            connectable: false,
            selectable: true,
            zIndex: 0,
          },
        ]
      : [];

    const cards: Node[] = l.nodes.map((n) => ({
      id: n.fqn,
      type: "card",
      position: { x: n.rect.x, y: n.rect.y },
      width: n.rect.w,
      height: n.rect.h,
      data: { label: n.label, kind: n.kind, summary: n.summary ?? "", fqn: n.fqn } as NodeData,
      class: `c4-node ${n.kind}`,
      draggable: false,
      connectable: false,
      zIndex: 1,
    }));

    const edges: Edge[] = l.edges.map((e, i) => ({
      id: `e${i}`,
      source: e.from,
      target: e.to,
      label: e.label || undefined,
      type: "polyline",
      data: { points: e.points, labelPos: e.label_pos ?? null, dashed: e.dashed },
      class: `c4-edge ${e.kind}`,
      selectable: false,
      markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14, color: e.kind === "trigger" ? "var(--k-callable)" : "var(--line-strong)" },
    }));

    return { nodes: [...frame, ...cards], edges };
  }

  const built = $derived(build(layout));
  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
  $effect(() => {
    nodes = built.nodes;
    edges = built.edges;
  });

  // The node a right-click opened the context menu on, anchored at the pointer.
  // `event` is kept so "Find usages" can position its popover where the click was.
  type MenuState = { fqn: string; kind: string; label: string; isBoundary: boolean; x: number; y: number; event: MouseEvent };
  let menu = $state<MenuState | null>(null);

  function onnodecontextmenu({ event, node }: { event: MouseEvent; node: Node }): void {
    event.preventDefault();
    const data = node.data as NodeData;
    menu = { fqn: data.fqn, kind: data.kind, label: data.label, isBoundary: node.type === "boundary", x: event.clientX, y: event.clientY, event };
  }
  const closeMenu = () => (menu = null);

  // CanvasMenu renders these rule-separated. A row's `run` closes over the opened
  // node — drill / go up the structure, jump to a flow, reveal the definition, or
  // list usages at the click point.
  function menuSections(m: MenuState): MenuSection[] {
    const sections: MenuSection[] = [];

    const nav: MenuItem[] = [];
    if (m.isBoundary) {
      if (onup) nav.push({ label: "Go up a level", run: () => onup?.() });
    } else if (DRILL[m.kind]) {
      nav.push({ label: DRILL[m.kind], run: () => onpick?.(m.fqn) });
    }
    if (nav.length) sections.push({ items: nav });

    const mFlows = flows?.get(m.fqn) ?? [];
    if (mFlows.length) {
      sections.push({
        label: "Flows",
        items: mFlows.map((f) => ({ label: f.name, run: () => onpick?.(f.fqn), icon: "▶", badge: f.triggered ? "triggered" : undefined })),
      });
    }

    sections.push({
      items: [
        { label: "Go to definition", run: () => onsource?.(m.fqn) },
        { label: "Find usages", run: () => onusages?.(m.fqn, m.event) },
      ],
    });
    return sections;
  }
</script>

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
  </SvelteFlow>

  <!-- Top-right toolbar: export the diagram. -->
  <div class="customise">
    <DiagramExport container={flowEl} {nodes} filename={exportName} />
  </div>

  {#if menu}
    {@const m = menu}
    <CanvasMenu kind={m.kind} label={m.label} x={m.x} y={m.y} sections={menuSections(m)} onclose={closeMenu} />
  {/if}
</div>

<style>
  .flow { position: relative; width: 100%; height: 100%; }

  /* Floating toolbar, top-right, clear of the minimap (bottom-right). */
  .customise {
    position: absolute;
    top: 0.7rem;
    right: 0.7rem;
    z-index: 5;
    display: flex;
    gap: 0.4rem;
  }
</style>
