<script lang="ts">
  // The data entity (ER) overlay: one SVG drawing every entity card and the
  // reference links between them at the engine's placed coordinates. No geometry
  // is computed here beyond the link elbows, which follow the same formula the
  // static SVG renderer uses (`render.rs::draw_data_link`) — entity rects and
  // sizes come from `pseudoscript-emit::layout_data_scene`.

  // The card header band and per-row advance, matching the Rust layout constants.
  const HDR = 40;
  const ROW_H = 22;

  import type { DataEntity as Entity, DataLink as Link, Rect } from "$lib/core/diagram-scene.js";

  // The Svelte Flow node `data` for the entity overlay (see DataModel build()).
  type Data = {
    entities: Entity[];
    links: Link[];
    width: number;
    height: number;
    onpick?: ((fqn: string) => void) | null;
  };

  type Props = { data: Data };
  let { data }: Props = $props();

  const byFqn = $derived(new Map(data.entities.map((e) => [e.fqn, e])));
  const rowBaseline = (r: Rect, i: number): number => r.y + HDR + i * ROW_H + 15;

  // The card-fill interior path: left edge square (so the accent base shows as a
  // left rule), right corners rounded — the same shape a C4 card draws.
  function fillPath(r: Rect): string {
    const rad = 8;
    const ix = r.x + 5;
    const right = r.x + r.w;
    const bottom = r.y + r.h;
    return `M${ix},${r.y} H${right - rad} A${rad},${rad} 0 0 1 ${right},${r.y + rad} V${bottom - rad} A${rad},${rad} 0 0 1 ${right - rad},${bottom} H${ix} Z`;
  }

  // An elbow from the referencing field's row to the left edge of the referenced
  // card: right out of the source row, across to the midpoint, down, and in.
  function elbow(link: Link): string {
    const from = byFqn.get(link.from);
    const to = byFqn.get(link.to);
    if (!from || !to) return "";
    const idx = Math.max(0, from.rows.findIndex((r) => r.name === link.field));
    const y1 = from.rect.y + HDR + idx * ROW_H + 11;
    const x1 = from.rect.x + from.rect.w;
    const x2 = to.rect.x;
    const y2 = to.rect.y + to.rect.h / 2;
    const midx = Math.round((x1 + x2) / 2);
    return `M${x1},${y1} H${midx} V${y2} H${x2}`;
  }
</script>

<svg class="data-er" width={data.width} height={data.height} viewBox="0 0 {data.width} {data.height}">
  <defs>
    <marker id="er-arrow" markerWidth="9" markerHeight="9" refX="8" refY="3" orient="auto">
      <path d="M0,0 L8,3 L0,6 z" />
    </marker>
  </defs>

  {#each data.links as link, i (i)}
    <path class="er-link" d={elbow(link)} marker-end="url(#er-arrow)" />
  {/each}

  {#each data.entities as e (e.fqn)}
    <g
      class="er-card"
      class:focal={e.focal}
      role="button"
      tabindex="-1"
      onclick={() => data.onpick?.(e.fqn)}
      onkeydown={(ev) => ev.key === "Enter" && data.onpick?.(e.fqn)}
    >
      <!-- accent base; card-fill interior leaving a left rule; neutral border -->
      <rect class="er-base" x={e.rect.x} y={e.rect.y} width={e.rect.w} height={e.rect.h} rx="8" />
      <path class="er-fill" d={fillPath(e.rect)} />
      <rect class="er-border" x={e.rect.x} y={e.rect.y} width={e.rect.w} height={e.rect.h} rx="8" />
      <text class="er-eyebrow" x={e.rect.x + 16} y={e.rect.y + 18}>{e.form.toUpperCase()}</text>
      <text class="er-name" x={e.rect.x + 16} y={e.rect.y + 33}>{e.label}</text>
      {#each e.rows as r, i (i)}
        <!-- field name in ink, `:` dimmed, type in the data accent (the editor's
             type colour, matching the reference arrow); a variant row is a type -->
        <text class="er-row" x={e.rect.x + 16} y={rowBaseline(e.rect, i)}>{#if r.ty}<tspan class="er-field">{r.name}</tspan><tspan class="er-sep-colon">:{" "}</tspan><tspan class="er-type">{r.ty}</tspan>{:else}<tspan class="er-type">{r.name}</tspan>{/if}</text>
        {#if r.target}
          <circle class="er-dot" cx={e.rect.x + e.rect.w - 9} cy={rowBaseline(e.rect, i) - 4} r="3.5" />
        {/if}
      {/each}
    </g>
  {/each}
</svg>

<style>
  .data-er {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .er-card {
    cursor: pointer;
  }
  .er-base {
    fill: var(--k-data);
  }
  .er-fill {
    fill: var(--surface);
  }
  .er-border {
    fill: none;
    stroke: var(--line);
  }
  .er-card.focal .er-border {
    stroke: var(--k-data);
  }
  .er-card:hover .er-border {
    stroke: var(--accent);
  }
  .er-eyebrow {
    fill: var(--k-data);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 1.5px;
  }
  .er-name {
    fill: var(--ink);
    font-family: var(--font-display);
    font-size: 15px;
    font-weight: 700;
  }
  .er-row {
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .er-field {
    fill: var(--ink);
  }
  .er-sep-colon {
    fill: var(--ink-faint);
  }
  .er-type {
    fill: var(--k-data);
  }
  .er-dot {
    fill: var(--k-data);
  }
  .er-link {
    fill: none;
    stroke: var(--line-strong);
  }
  #er-arrow path {
    fill: var(--line-strong);
  }
</style>
