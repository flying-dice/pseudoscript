<script lang="ts">
  // A sequence participant, drawn from the engine's placed coordinates: the
  // kind-coloured C4 head card, the dashed lifeline, and the execution-activation
  // bar spanning its involvement. The node fills the canvas, so every coordinate
  // here is absolute (no geometry is computed in the component).

  import type { MenuRequest } from "$lib/core/types.js";

  // A placed participant lifeline from the layout engine (PlacedParticipant).
  type Placed = {
    id: string;
    label: string;
    kind: string;
    card: { x: number; y: number; w: number; h: number };
    lifeline_x: number;
    top: number;
    bottom: number;
  };
  // A placed activation bar from the layout engine (Activation).
  type Activation = {
    participant: string;
    x: number;
    top: number;
    bottom: number;
    owner: boolean;
  };
  // The synthesised initiator head: a trigger kind + its title.
  type Initiator = { kind: string; title: string };

  type LifelineData = {
    placed: Placed;
    act?: Activation | null;
    onmenu?: MenuRequest | null;
  };

  type Props = {
    data: LifelineData;
  };

  let { data }: Props = $props();

  const p = data.placed;
  const act = data.act;

  // A synthesised initiator is not a declared node — it's the trigger that drives
  // the entry: `event:<FQN>` (#[onevent]), `client` (#[http]), `scheduler`
  // (#[schedule]), or a generic `caller` (a direct/untriggered call). The
  // projector tags these as `person` for layout, but they aren't people — show
  // the trigger kind as the eyebrow and a neutral card, not a person card.
  function initiatorHead(id: string | null | undefined): Initiator | null {
    if (!id) return null;
    if (id.startsWith("event:")) return { kind: "onevent", title: id.slice(6) };
    if (id === "client") return { kind: "http", title: "client" };
    if (id === "scheduler") return { kind: "schedule", title: "scheduler" };
    if (id === "caller") return { kind: "caller", title: "caller" };
    return null;
  }
  const initiator = initiatorHead(p.id);
  // Eyebrow + title + card-kind class: a declared node keeps its C4 kind/name; a
  // synthetic initiator reads its trigger and renders as a neutral `.initiator`.
  const kindLabel = initiator ? initiator.kind : p.kind;
  const nameLabel = initiator ? initiator.title : p.label;
  const cardKind = initiator ? "initiator" : p.kind;

  // Canvas interaction mirrors the C4 graph: right-click opens the actions menu
  // (go-to-definition / find-usages). Only declared nodes participate; synthesised
  // trigger actors (client/scheduler/event) have no resolvable symbol.
  const interactive = !!p.id && !initiator;
  const oncontextmenu = (e: MouseEvent) => {
    if (!interactive) return;
    e.preventDefault();
    data.onmenu?.({ fqn: p.id, kind: cardKind, label: nameLabel }, e);
  };
</script>

<div class="seq-life">
  <div
    class="seq-card c4-node {cardKind}"
    class:interactive
    style="left:{p.card.x}px; top:{p.card.y}px; width:{p.card.w}px; height:{p.card.h}px"
    role="button"
    tabindex="-1"
    {oncontextmenu}
  >
    <span class="seq-kind">{kindLabel}</span>
    <span class="seq-name">{nameLabel}</span>
  </div>

  <svg class="seq-overlay" width="100%" height="100%">
    <!-- dashed lifeline -->
    <line x1={p.lifeline_x} y1={p.top} x2={p.lifeline_x} y2={p.bottom} class="seq-lifeline" />

    <!-- execution-activation bar -->
    {#if act}
      <rect
        class="seq-act {act.owner ? 'seq-act-owner' : ''}"
        x={act.x - 5}
        y={act.top}
        width="10"
        height={Math.max(10, act.bottom - act.top)}
        rx="2"
      />
    {/if}
  </svg>
</div>

<style>
  .seq-life {
    position: relative;
    width: 100%;
    height: 100%;
    background: transparent;
    pointer-events: none;
  }
  /* the C4 head card: kind eyebrow over the name, a kind-coloured left stripe */
  .seq-card {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.15rem;
    padding: 0 0.7rem;
    overflow: hidden;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-left: 3px solid var(--k, var(--ink-faint));
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
  }
  .seq-card.interactive { pointer-events: auto; cursor: pointer; }
  .seq-card.interactive:hover { border-color: var(--accent); }
  .seq-card.person { --k: var(--k-person); }
  .seq-card.system { --k: var(--k-system); }
  .seq-card.container { --k: var(--k-container); }
  .seq-card.component { --k: var(--k-component); }
  .seq-card.data { --k: var(--k-data); }
  .seq-card.callable { --k: var(--k-callable); }
  /* a synthesised trigger initiator (onevent/http/schedule/caller): not a
     declared node, so it stays neutral rather than wearing the person colour */
  .seq-card.initiator { --k: var(--ink-faint); }
  .seq-card.initiator .seq-kind { font-style: italic; }
  .seq-kind {
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--k, var(--ink-faint));
  }
  .seq-name {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .seq-overlay {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .seq-lifeline {
    stroke: var(--line-strong);
    stroke-dasharray: 2 4;
  }
  .seq-act {
    fill: var(--surface-3);
    stroke: var(--line-strong);
  }
  .seq-act-owner {
    fill: color-mix(in srgb, var(--accent) 18%, transparent);
    stroke: color-mix(in srgb, var(--accent) 55%, transparent);
  }
</style>
