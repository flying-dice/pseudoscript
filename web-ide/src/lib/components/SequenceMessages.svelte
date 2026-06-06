<script lang="ts">
  // The message layer: one overlay drawing every message at the engine's placed
  // coordinates. Calls are solid + numbered; returns are dashed and coloured by
  // their Ok/Err marker with the payload as `<type>`; self-messages loop on their
  // lifeline. No geometry is computed here — only the absolute coords are drawn.

  import type { MenuRequest } from "$lib/core/types.js";

  // A positioned message from the `pseudoscript-layout` crate: the pre-layout
  // call/return shape plus the absolute coordinates the engine placed it at.
  type Message = {
    kind: string;
    from: string;
    to: string;
    label?: string;
    detail?: string;
    from_x: number;
    to_x: number;
    y: number;
    dir: number;
    step: number;
  };

  // The Svelte Flow node `data` for the message overlay (see FlowTimeline build()).
  type Data = {
    messages: Message[];
    width: number;
    height: number;
    lifelineX: Record<string, number>;
    onmenu?: MenuRequest | null;
    typeFqn?: Record<string, string> | null;
  };

  // A return's resolved colour + glyph.
  type Return = { color: string; text: string };
  // One run of a split signature: its text and the declared-type fqn (or null).
  type TypePart = { text: string; fqn: string | null };

  type Props = { data: Data };

  let { data }: Props = $props();

  const labelW = (s: string | undefined): number => (s ? s.length * 7.8 + 8 : 0);

  // A return's colour + glyph, by marker.
  function ret(marker: string | undefined): Return {
    if (marker === "Ok" || marker === "Some") return { color: "var(--seq-ok)", text: `↩ ${marker}` };
    if (marker === "Err" || marker === "None") return { color: "var(--seq-err)", text: `↩ ${marker}` };
    return { color: "var(--ink-faint)", text: marker ? `↩ ${marker}` : "↩ return" };
  }

  // A marked return shows its payload as a generic argument (`Ok<Order>`); a bare
  // value return shows the whole type as a spaced suffix.
  const retType = (m: Message): string => (m.detail ? (m.label ? `<${m.detail}>` : ` ${m.detail}`) : "");
  // The badge sits just left of the source lifeline (centre − activation − gap).
  const badgeX = (m: Message): number => (data.lifelineX[m.from] ?? m.from_x) - 17;

  // A call/self message targets a member callable; right-click opens its menu
  // (go-to-definition / find-usages), matching the lifeline and C4 graph.
  const callee = (m: Message): string => (m.kind === "self" ? `${m.from}::${m.label}` : `${m.to}::${m.label}`);
  const onLabelMenu = (m: Message, e: MouseEvent): void => {
    e.preventDefault();
    data.onmenu?.({ fqn: callee(m), kind: "callable", label: m.label ?? "" }, e);
  };
  const onTypeMenu = (part: TypePart, e: MouseEvent): void => {
    if (!part.fqn) return;
    e.preventDefault();
    data.onmenu?.({ fqn: part.fqn, kind: "data", label: part.text }, e);
  };

  // Split a signature/return-type string into identifier and separator runs,
  // tagging each identifier that names a declared `data` type with its FQN so it
  // becomes hoverable. Built-ins (Result, string, …) carry no fqn.
  const typeParts = (detail: string | undefined): TypePart[] =>
    (detail ?? "")
      .split(/([A-Za-z_][A-Za-z0-9_]*)/)
      .map((text, i) => ({ text, fqn: i % 2 === 1 ? (data.typeFqn?.[text] ?? null) : null }))
      .filter((p) => p.text !== "");
</script>

<svg class="seq-messages" width={data.width} height={data.height} viewBox="0 0 {data.width} {data.height}">
  {#each data.messages as m, i (i)}
    {#if m.kind === "call"}
      <line class="seq-call-line" x1={m.from_x} y1={m.y} x2={m.to_x} y2={m.y} />
      <path class="seq-call-head" d="M{m.to_x - m.dir * 7},{m.y - 4} L{m.to_x},{m.y} L{m.to_x - m.dir * 7},{m.y + 4} z" />
      {#if m.label}
        {@const mx = (m.from_x + m.to_x) / 2}
        {@const full = m.label + (m.detail ?? "")}
        <rect class="seq-pill" x={mx - labelW(full) / 2} y={m.y - 19} width={labelW(full)} height="14" rx="4" />
        <text class="seq-call-text" x={mx} y={m.y - 9} text-anchor="middle"><tspan
            class="seq-hit"
            role="button"
            tabindex="-1"
            oncontextmenu={(e) => onLabelMenu(m, e)}
          >{m.label}</tspan>{#each typeParts(m.detail) as part, pi (pi)}{#if part.fqn}<tspan class="seq-type seq-type-link" role="button" tabindex="-1" oncontextmenu={(e) => onTypeMenu(part, e)}>{part.text}</tspan>{:else}<tspan class="seq-type">{part.text}</tspan>{/if}{/each}</text>
      {/if}
      <circle class="seq-call-dot" cx={badgeX(m)} cy={m.y} r="8" />
      <text class="seq-call-num" x={badgeX(m)} y={m.y + 3} text-anchor="middle">{m.step}</text>
    {:else if m.kind === "self"}
      {@const lx = m.from_x + 5}
      <path class="seq-self" d="M{lx},{m.y} h34 a6 6 0 0 1 6 6 v8 a6 6 0 0 1 -6 6 h-34" fill="none" />
      <path class="seq-self-head" d="M{lx + 7},{m.y + 16} L{lx},{m.y + 20} L{lx + 7},{m.y + 24}" fill="none" />
      <text
        class="seq-label seq-hit"
        x={lx + 46}
        y={m.y + 4}
        text-anchor="start"
        role="button"
        tabindex="-1"
        oncontextmenu={(e) => onLabelMenu(m, e)}>{m.label}</text>
      <circle class="seq-call-dot" cx={badgeX(m)} cy={m.y} r="8" />
      <text class="seq-call-num" x={badgeX(m)} y={m.y + 3} text-anchor="middle">{m.step}</text>
    {:else}
      {@const r = ret(m.label)}
      {@const mx = (m.from_x + m.to_x) / 2}
      {@const type = retType(m)}
      {@const full = r.text + type}
      <line class="seq-ret-line" x1={m.from_x} y1={m.y} x2={m.to_x} y2={m.y} stroke={r.color} />
      <path class="seq-ret-head" d="M{m.to_x - m.dir * 7},{m.y - 4} L{m.to_x},{m.y} L{m.to_x - m.dir * 7},{m.y + 4}" stroke={r.color} />
      <rect class="seq-pill" x={mx - labelW(full) / 2} y={m.y - 19} width={labelW(full)} height="14" rx="4" />
      <text class="seq-ret-text" x={mx} y={m.y - 9} text-anchor="middle" fill={r.color}
        >{r.text}{#each typeParts(type) as part, pi (pi)}{#if part.fqn}<tspan class="seq-type seq-type-link" role="button" tabindex="-1" oncontextmenu={(e) => onTypeMenu(part, e)}>{part.text}</tspan>{:else}<tspan class="seq-type">{part.text}</tspan>{/if}{/each}</text>
    {/if}
  {/each}
</svg>

<style>
  .seq-messages {
    position: absolute;
    inset: 0;
    overflow: visible;
    pointer-events: none;
  }
  /* call/self labels are members — interactive even though the overlay isn't */
  .seq-hit { pointer-events: auto; cursor: pointer; }
  .seq-hit:hover { fill: var(--accent); }
  /* data-type tokens within a signature are interactive too */
  .seq-type-link { pointer-events: auto; cursor: pointer; }
  .seq-type-link:hover { fill: var(--accent); text-decoration: underline; }
  .seq-call-line { stroke: var(--ink); stroke-width: 1.4; }
  .seq-call-head { fill: var(--ink); }
  .seq-ret-line { stroke-width: 1.3; stroke-dasharray: 5 3; }
  .seq-ret-head { fill: none; stroke-width: 1.3; }
  .seq-pill { fill: var(--surface); fill-opacity: 0.92; }
  .seq-call-text { fill: var(--ink); font-family: var(--font-mono); font-size: 12.5px; }
  .seq-ret-text { font-family: var(--font-mono); font-size: 12.5px; }
  .seq-type { fill: var(--ink-faint); }
  .seq-self { stroke: var(--ink-soft); stroke-width: 1.4; }
  .seq-self-head { stroke: var(--ink-soft); stroke-width: 1.4; }
  .seq-label { fill: var(--ink-soft); font-family: var(--font-mono); font-size: 11px; }
  .seq-call-dot { fill: var(--accent); }
  .seq-call-num { fill: var(--accent-ink); font-family: var(--font-mono); font-size: 10px; font-weight: 700; }
</style>
