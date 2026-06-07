<script lang="ts">
  // The feature flow overlay: a scenario's ordered given/when/then steps as
  // connected boxes, top to bottom, at the engine's placed coordinates
  // (`pseudoscript-emit::layout_feature_scene`). No geometry is computed here.

  import type { FeatureStep as Step } from "$lib/core/diagram-scene.js";

  // The Svelte Flow node `data` for the step overlay (see FeatureFlow build()).
  type Data = {
    name: string;
    targetLabel: string;
    steps: Step[];
    width: number;
    height: number;
  };

  type Props = { data: Data };
  let { data }: Props = $props();

  // A vertical connector from one step box to the next.
  type Connector = { x: number; y1: number; y2: number };
  const connectors = $derived<Connector[]>(
    data.steps.slice(0, -1).map((s, i) => {
      const a = s.rect;
      const b = data.steps[i + 1].rect;
      return { x: a.x + a.w / 2, y1: a.y + a.h, y2: b.y };
    }),
  );

  // Text geometry, matching the Rust layout (`render.rs`) so the wrapped line
  // count agrees with the box height the engine computed.
  const PAD_X = 16;
  const TEXT_TOP = 36;
  const LINE_H = 17;
  const CHAR_W = 8;

  // Greedy word-wrap to the box's inner width; an over-long word keeps its line.
  function wrap(text: string, boxW: number): string[] {
    const max = Math.max(1, Math.floor((boxW - 2 * PAD_X) / CHAR_W));
    const lines: string[] = [];
    let cur = "";
    for (const word of text.split(/\s+/).filter(Boolean)) {
      if (!cur) cur = word;
      else if (cur.length + 1 + word.length <= max) cur += ` ${word}`;
      else {
        lines.push(cur);
        cur = word;
      }
    }
    if (cur) lines.push(cur);
    return lines.length ? lines : [""];
  }
</script>

<svg class="feat" width={data.width} height={data.height} viewBox="0 0 {data.width} {data.height}">
  <defs>
    <marker id="feat-arrow" markerWidth="9" markerHeight="9" refX="8" refY="3" orient="auto">
      <path d="M0,0 L8,3 L0,6 z" />
    </marker>
  </defs>

  <text class="feat-eyebrow" x="20" y="22">FEATURE</text>
  <text class="feat-name" x="20" y="44">{data.name}</text>
  <text class="feat-target" x="20" y="60">for {data.targetLabel}</text>

  {#each connectors as c, i (i)}
    <line class="feat-link" x1={c.x} y1={c.y1} x2={c.x} y2={c.y2} marker-end="url(#feat-arrow)" />
  {/each}

  {#each data.steps as s, i (i)}
    <g class="feat-step kw-{s.keyword}">
      <rect class="feat-rect" x={s.rect.x} y={s.rect.y} width={s.rect.w} height={s.rect.h} rx="8" />
      <rect class="feat-rule" x={s.rect.x} y={s.rect.y} width="5" height={s.rect.h} rx="2" />
      <text class="feat-kw" x={s.rect.x + 16} y={s.rect.y + 18}>{s.keyword.toUpperCase()}</text>
      <text class="feat-text" x={s.rect.x + PAD_X} y={s.rect.y + TEXT_TOP}
        >{#each wrap(s.text, s.rect.w) as line, li (li)}<tspan x={s.rect.x + PAD_X} dy={li === 0 ? 0 : LINE_H}>{line}</tspan>{/each}</text
      >
    </g>
  {/each}
</svg>

<style>
  .feat {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .feat-eyebrow {
    fill: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 2px;
  }
  .feat-name {
    fill: var(--ink);
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 700;
  }
  .feat-target {
    fill: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 11.5px;
  }
  .feat-rect {
    fill: var(--surface);
    stroke: var(--line);
  }
  /* per-keyword accent: the left rule and keyword eyebrow */
  .feat-step {
    --kc: var(--ink-faint);
  }
  .kw-given {
    --kc: var(--k-person);
  }
  .kw-when {
    --kc: var(--k-component);
  }
  .kw-then {
    --kc: var(--k-container);
  }
  .feat-rule {
    fill: var(--kc);
  }
  .feat-kw {
    fill: var(--kc);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 1.5px;
  }
  .feat-text {
    fill: var(--ink);
    font-family: var(--font-mono);
    font-size: 13px;
  }
  .feat-link {
    stroke: var(--ink-faint);
  }
  #feat-arrow path {
    fill: var(--ink-faint);
  }
</style>
