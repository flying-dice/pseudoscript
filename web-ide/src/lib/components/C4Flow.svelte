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
  import { Background, Controls, MarkerType, MiniMap, SvelteFlow, ViewportPortal } from "@xyflow/svelte";
  import type { Edge, Node } from "@xyflow/svelte";
  import { Lock, LockOpen, RotateCcw } from "@lucide/svelte";
  import BoundaryNode from "./BoundaryNode.svelte";
  import C4Node from "./C4Node.svelte";
  import CanvasMenu from "./CanvasMenu.svelte";
  import DiagramExport from "./DiagramExport.svelte";
  import LayoutControl from "./LayoutControl.svelte";
  import PolylineEdge from "./PolylineEdge.svelte";
  import { theme } from "$lib/theme.svelte.js";
  import { buildC4Dot } from "$lib/dot-source.js";
  import { cellAt } from "$lib/core/pins.js";
  import type { LayoutTweaks, MenuItem, MenuSection } from "$lib/core/types.js";

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
    // Merged labels of parallel same-direction relationships, sorted and
    // de-duplicated; empty for a trigger/provenance edge. Stacked one per line.
    labels: string[];
    points: Pt[];
    label_pos?: Pt | null;
    dashed: boolean;
  };
  type BoundaryFrame = { fqn: string; title: string; kind: string; rect: Rect };
  // The grid geometry, present only for an experimental-grid layout; lets a drop
  // pixel map back to a cell (drag-to-pin). Cell (r,c) centres at
  // `origin + (c·cell_w, r·cell_h)`.
  type GridInfo = { cols: number; rows: number; cell_w: number; cell_h: number; origin: Pt };
  type Layout = {
    width: number;
    height: number;
    nodes: LaidOutNode[];
    edges: LaidOutEdge[];
    // Enclosing frames, outermost first: one for a container view, two nested
    // (system then container) for a component view.
    boundaries?: BoundaryFrame[];
    grid?: GridInfo | null;
  };
  // The `data` payload carried by every Svelte Flow node (card or boundary).
  type NodeData = {
    label: string;
    kind: string;
    summary: string;
    fqn: string;
    boundary?: boolean;
    onclose?: () => void;
    // Grid mode: pinned marker, unlocked flag, and clear-this-pin callback.
    pinned?: boolean;
    unlocked?: boolean;
    onunpin?: () => void;
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
    tweaks?: LayoutTweaks | null;
    onlayoutchange?: ((tweaks: LayoutTweaks) => void) | null;
    // Drag-to-pin: when `unlocked`, cards are draggable and a drop snaps to a grid
    // cell, reported via `onpin`. `onunlock` toggles the mode.
    unlocked?: boolean;
    onpin?: ((fqn: string, row: number, col: number) => void) | null;
    onunlock?: ((next: boolean) => void) | null;
    // The FQNs pinned in this view (marks their cards), the clear-one callback, and
    // the reset-this-diagram callback.
    pinnedFqns?: Set<string> | null;
    onunpin?: ((fqn: string) => void) | null;
    onresetgrid?: (() => void) | null;
    onuniverse?: ((fqn: string) => void) | null;
  };

  let {
    scene,
    layout,
    onpick,
    onup,
    flows = null,
    onsource = null,
    onusages = null,
    tweaks = null,
    onlayoutchange = null,
    unlocked = false,
    onpin = null,
    onunlock = null,
    pinnedFqns = null,
    onunpin = null,
    onresetgrid = null,
    onuniverse = null,
  }: Props = $props();

  // Drag-to-pin is offered only for a grid layout (the engine emits `grid` only
  // then); leaving grid mode or switching to a non-grid view hides it.
  const gridEditable = $derived(!!layout?.grid && !!onpin && !!onunlock);
  const dragging = $derived(gridEditable && unlocked);
  // Whether this view has any manual placements (gates the Reset control).
  const hasPins = $derived((pinnedFqns?.size ?? 0) > 0);

  // The grid lines to draw while editing, in flow coordinates. Cell (r,c) centres
  // at `origin + (c·cell_w, r·cell_h)`, so boundaries sit half a cell out.
  const gridOverlay = $derived.by(() => {
    const g = layout?.grid;
    if (!dragging || !g) return null;
    return {
      left: g.origin.x - g.cell_w / 2,
      top: g.origin.y - g.cell_h / 2,
      w: g.cols * g.cell_w,
      h: g.rows * g.cell_h,
      vx: Array.from({ length: g.cols + 1 }, (_, c) => c * g.cell_w),
      hy: Array.from({ length: g.rows + 1 }, (_, r) => r * g.cell_h),
    };
  });

  // Drive Svelte Flow's colour mode from the app theme so the canvas (pane,
  // grid, minimap, controls) follows light/dark instead of being pinned dark.
  const colorMode = $derived(theme.resolved === "light" ? "light" : "dark");

  // The canvas root, captured for diagram export.
  let flowEl = $state<HTMLDivElement | null>(null);
  // Download name: the boundary view's subject, else a generic fallback.
  const exportName = $derived((scene.of ?? "").split("::").pop() || "diagram");

  const nodeTypes = { boundary: BoundaryNode, card: C4Node };
  const edgeTypes = { polyline: PolylineEdge };

  // The Graphviz `dot` source for the current view (the engine's layout input),
  // offered as a download so the graph can be checked against real `dot`.
  const dotSource = $derived(
    layout ? () => buildC4Dot(layout, tweaks?.orientation === "lr") : null,
  );

  // Debug-only affordance, gated on `__debug=true` in the URL: render the current
  // view's DOT through real Graphviz (compiled to wasm) and open it in a new tab,
  // so our layout can be eyeballed against the reference engine.
  const showGraphvizDebug =
    typeof window !== "undefined" && window.location.href.includes("__debug=true");

  async function openGraphviz(): Promise<void> {
    if (!dotSource) return;
    // Open the tab inside the click gesture so the pop-up isn't blocked, then
    // fill it once the (lazy-loaded) engine has rendered.
    const win = window.open("", "_blank");
    try {
      const { Graphviz } = await import("@hpcc-js/wasm-graphviz");
      const graphviz = await Graphviz.load();
      const svg = graphviz.dot(dotSource());
      if (win) {
        win.document.write(
          `<!doctype html><html><head><meta charset="utf-8"><title>Graphviz — ${exportName}</title></head><body style="margin:0;background:#fff">${svg}</body></html>`,
        );
        win.document.close();
      }
    } catch (e) {
      win?.close();
      console.error("[graphviz-debug] render failed", e);
    }
  }

  // Which deeper view a node drills into, by kind. Persons / components have no
  // structural view below them, so they get no drill button (info only).
  const DRILL: Record<string, string> = { system: "Open container diagram", container: "Open component diagram" };

  // Map the positioned layout into Svelte Flow nodes + edges. The boundary frame
  // sits behind (zIndex 0) as a non-interactive box; cards sit on top at their
  // engine-computed rect (position + size); edges follow the routed polylines.
  function build(l: Layout | null): { nodes: Node[]; edges: Edge[] } {
    if (!l || !Array.isArray(l.nodes)) return { nodes: [], edges: [] };

    // Frames sit behind the cards (zIndex 0), outermost first so a nested inner
    // frame draws over its outer one. Only the view's own anchor frame
    // (`fqn === scene.of`) carries the close button that pops up one level.
    const frame: Node[] = (l.boundaries ?? []).map((b, i) => ({
      id: `__boundary_${i}`,
      type: "boundary",
      position: { x: b.rect.x, y: b.rect.y },
      width: b.rect.w,
      height: b.rect.h,
      data: {
        label: b.title,
        kind: b.kind,
        summary: "",
        fqn: b.fqn,
        boundary: true,
        onclose: b.fqn === scene.of ? (onup ?? undefined) : undefined,
      } as NodeData,
      class: `c4-boundary ${b.kind}`,
      draggable: false,
      connectable: false,
      selectable: true,
      zIndex: 0,
    }));

    const cards: Node[] = l.nodes.map((n) => {
      const pinned = !!pinnedFqns?.has(n.fqn);
      return {
        id: n.fqn,
        type: "card",
        position: { x: n.rect.x, y: n.rect.y },
        width: n.rect.w,
        height: n.rect.h,
        data: {
          label: n.label,
          kind: n.kind,
          summary: n.summary ?? "",
          fqn: n.fqn,
          pinned,
          unlocked: dragging,
          onunpin: pinned ? () => onunpin?.(n.fqn) : undefined,
        } as NodeData,
        class: `c4-node ${n.kind}`,
        draggable: dragging,
        connectable: false,
        zIndex: 1,
      };
    });

    const edges: Edge[] = l.edges.map((e, i) => ({
      id: `e${i}`,
      source: e.from,
      target: e.to,
      label: e.labels.join("\n") || undefined,
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

  // Drag-to-pin: snap a dropped card to the nearest grid cell and report it. The
  // re-layout (driven by the new pin) repositions the card exactly on that cell.
  function onnodedragstop({ targetNode }: { targetNode: Node | null }): void {
    const g = layout?.grid;
    if (!targetNode || targetNode.type === "boundary" || !g || !onpin) return;
    const w = targetNode.width ?? targetNode.measured?.width ?? 0;
    const h = targetNode.height ?? targetNode.measured?.height ?? 0;
    const { row, col } = cellAt(g, targetNode.position.x + w / 2, targetNode.position.y + h / 2);
    onpin(targetNode.id, row, col);
  }

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
    if (onuniverse) {
      sections.push({ items: [{ label: "Show in 3D graph", run: () => onuniverse?.(m.fqn), icon: "✦" }] });
    }
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
    nodesDraggable={dragging}
    nodesConnectable={false}
    elementsSelectable={dragging}
    proOptions={{ hideAttribution: true }}
    {onnodecontextmenu}
    {onnodedragstop}
  >
    <Background gap={24} />
    {#if gridOverlay}
      {@const o = gridOverlay}
      <ViewportPortal target="back">
        <svg
          class="grid-overlay"
          style="position:absolute; left:{o.left}px; top:{o.top}px;"
          width={o.w}
          height={o.h}
        >
          {#each o.vx as x (x)}<line x1={x} y1={0} x2={x} y2={o.h} />{/each}
          {#each o.hy as y (y)}<line x1={0} y1={y} x2={o.w} y2={y} />{/each}
        </svg>
      </ViewportPortal>
    {/if}
    <MiniMap pannable zoomable />
    <Controls showLock={false} />
  </SvelteFlow>

  <!-- Top-right toolbar: layout tweaks + export the diagram. -->
  <div class="customise">
    {#if gridEditable}
      <button
        class="export-trigger"
        class:active={unlocked}
        data-testid="grid-lock"
        onclick={() => onunlock?.(!unlocked)}
        title={unlocked ? "Lock the grid (stop dragging)" : "Unlock the grid to drag nodes between cells"}
      >
        {#if unlocked}
          <LockOpen size={13} strokeWidth={2} aria-hidden="true" /> Unlocked
        {:else}
          <Lock size={13} strokeWidth={2} aria-hidden="true" /> Locked
        {/if}
      </button>
      {#if hasPins && onresetgrid}
        <button
          class="export-trigger"
          data-testid="grid-reset"
          onclick={() => onresetgrid?.()}
          title="Reset this diagram's manual placements (back to auto-layout)"
        >
          <RotateCcw size={13} strokeWidth={2} aria-hidden="true" /> Reset
        </button>
      {/if}
    {/if}
    {#if tweaks && onlayoutchange}
      <LayoutControl {tweaks} onchange={onlayoutchange} />
    {/if}
    {#if showGraphvizDebug && dotSource}
      <button
        class="export-trigger"
        onclick={openGraphviz}
        title="Render this view through real Graphviz (debug)"
      >Graphviz</button>
    {/if}
    <DiagramExport container={flowEl} {nodes} filename={exportName} dotSource={dotSource} />
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

  /* The unlock toggle reads as "armed" while dragging is enabled. */
  .customise :global(.export-trigger.active) {
    border-color: var(--k-callable, #2563eb);
    color: var(--k-callable, #2563eb);
  }

  /* The snap grid shown while editing — drawn in flow space behind the cards. */
  .grid-overlay {
    overflow: visible;
    pointer-events: none;
  }
  .grid-overlay line {
    stroke: var(--k-callable, #2563eb);
    stroke-width: 1;
    opacity: 0.28;
  }
</style>
