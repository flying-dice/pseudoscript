<script lang="ts">
  // The behavioural lens: a triggered entry point's call sequence drawn as a UML
  // sequence diagram. All positioning is done by the `pseudoscript-layout` crate
  // (shared with the static SVG renderer); this component is a dumb renderer of
  // the positioned `Layout` — it computes no geometry. Participants are lifeline
  // nodes (kind-coloured C4 head card, dashed lifeline, activation bar), messages
  // are a single overlay of arrows, and alt/loop render as combined fragments.
  import { Background, Controls, MiniMap, SvelteFlow } from "@xyflow/svelte";
  import type { Edge, Node } from "@xyflow/svelte";
  import SequenceLifeline from "./SequenceLifeline.svelte";
  import SequenceFragment from "./SequenceFragment.svelte";
  import SequenceMessages from "./SequenceMessages.svelte";
  import DiagramExport from "./DiagramExport.svelte";
  import { theme } from "$lib/theme.svelte.js";

  // The triggered scene this flow projects; only `entry` is read here for the
  // header label (the rest is consumed by the layout engine upstream).
  type Scene = { entry?: string | null };
  // A positioned participant lifeline: its column x and node id.
  type Participant = { id: string; lifeline_x: number };
  // A participant's activation bar, keyed by participant id.
  type Activation = { participant: string };
  // A combined fragment box (alt / loop) with its absolute rect and dividers.
  type Fragment = {
    kind: string;
    label: string;
    rect: { x: number; y: number; w: number; h: number };
    dividers: { guard: string; y: number }[];
  };
  // The positioned layout produced by the `pseudoscript-layout` crate.
  type Layout = {
    width: number;
    height: number;
    participants: Participant[];
    activations?: Activation[];
    fragments: Fragment[];
    messages: unknown;
  };

  // A hover/usages callback fired with a symbol fqn and the originating event.
  type SymbolHandler = (fqn: string, event: MouseEvent) => void;

  type Props = {
    scene: Scene | null;
    layout: Layout | null;
    oninfo?: SymbolHandler | null;
    oninfoend?: (() => void) | null;
    onusages?: SymbolHandler | null;
    typeFqn?: string | null;
  };

  let { scene, layout, oninfo = null, oninfoend = null, onusages = null, typeFqn = null }: Props = $props();

  // Drive Svelte Flow's colour mode from the app theme so the canvas follows
  // light/dark instead of being pinned dark.
  const colorMode = $derived(theme.resolved === "light" ? "light" : "dark");

  const nodeTypes = {
    lifeline: SequenceLifeline,
    fragment: SequenceFragment,
    messages: SequenceMessages,
  };

  const leaf = (fqn: string | null | undefined): string | undefined => (fqn ?? "").split("::").pop();

  // Map the positioned Layout into Svelte Flow nodes. Fragments sit behind
  // (zIndex 0), lifelines in the middle, the message overlay on top.
  function build(l: Layout | null): { nodes: Node[]; edges: Edge[] } {
    if (!l || !Array.isArray(l.participants)) return { nodes: [], edges: [] };
    const actByPid = new Map<string, Activation>((l.activations ?? []).map((a) => [a.participant, a]));
    const lifelineX = Object.fromEntries(l.participants.map((p) => [p.id, p.lifeline_x]));

    const nodes: Node[] = [
      ...l.fragments.map((f: Fragment, k: number) => ({
        id: `frag${k}`,
        type: "fragment",
        position: { x: f.rect.x, y: f.rect.y },
        width: f.rect.w,
        height: f.rect.h,
        // Divider y's are absolute; make them relative to the fragment box.
        data: {
          kind: f.kind,
          label: f.label,
          dividers: f.dividers.map((d: { guard: string; y: number }) => ({ guard: d.guard, y: d.y - f.rect.y })),
        },
        class: "seq-shell",
        draggable: false,
        selectable: false,
        connectable: false,
        // Above the lifelines/activation bars so the operator tab, guard, and
        // else labels read over them (the box fill is faint enough to see through).
        zIndex: 2,
      })),
      ...l.participants.map((p: Participant) => ({
        id: p.id,
        type: "lifeline",
        // The node spans the canvas; the card sits at its absolute x inside.
        position: { x: 0, y: 0 },
        width: l.width,
        height: l.height,
        data: { placed: p, act: actByPid.get(p.id) ?? null, oninfo, oninfoend, onusages },
        class: "seq-shell",
        draggable: false,
        selectable: false,
        connectable: false,
        zIndex: 1,
      })),
      {
        id: "__messages",
        type: "messages",
        position: { x: 0, y: 0 },
        width: l.width,
        height: l.height,
        data: { messages: l.messages, width: l.width, height: l.height, lifelineX, oninfo, oninfoend, onusages, typeFqn },
        class: "seq-shell",
        draggable: false,
        selectable: false,
        connectable: false,
        zIndex: 3,
      },
    ];
    return { nodes, edges: [] };
  }

  const built = $derived(build(layout));
  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);

  // The canvas root, captured for diagram export; download name from the entry.
  let flowEl = $state<HTMLDivElement | null>(null);
  const exportName = $derived(leaf(scene?.entry) || "sequence");
  $effect(() => {
    nodes = built.nodes;
    edges = built.edges;
  });
</script>

<div class="timeline">
  <header class="flow-head">
    <div class="title">
      <span class="kicker">flow</span>
      <span class="name">{leaf(scene?.entry)}</span>
    </div>
    <span class="hint">sequence diagram — scroll to zoom · drag to pan</span>
    <DiagramExport container={flowEl} {nodes} filename={exportName} />
  </header>

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
  </div>
</div>

<style>
  .timeline {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .flow-head {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.7rem 1rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .title { display: flex; align-items: baseline; gap: 0.5rem; flex: none; }
  .title .kicker {
    font-family: var(--font-mono);
    font-size: 0.56rem;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .title .name {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1rem;
    color: var(--ink);
  }
  .hint {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.04em;
    color: var(--ink-faint);
  }
  .flow { flex: 1; min-height: 0; }
</style>
