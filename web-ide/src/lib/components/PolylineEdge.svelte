<script lang="ts">
  // A C4 relationship edge that follows the engine's routed polyline. The points
  // come from `pseudoscript-emit`'s layout (the same path the static SVG draws),
  // in the canvas coordinate space the nodes are positioned in, so the edge is
  // drawn verbatim — no client-side routing. Dashed marks a `from`-provenance
  // edge, matching the SVG.
  import { BaseEdge } from "@xyflow/svelte";
  import type { EdgeProps } from "@xyflow/svelte";

  type Pt = { x: number; y: number };

  let { markerEnd, label, data }: EdgeProps = $props();

  const geom = $derived.by(() => {
    const points = (data?.points as Pt[] | undefined) ?? [];
    if (points.length < 2) return null;
    const path = points.map((p, i) => `${i === 0 ? "M" : "L"}${p.x},${p.y}`).join(" ");
    // The engine's label anchor, else the polyline's midpoint.
    const labelPos = (data?.labelPos as Pt | null | undefined) ?? points[Math.floor(points.length / 2)];
    return { path, labelX: labelPos.x, labelY: labelPos.y };
  });

  const dash = $derived(data?.dashed ? "4 3" : undefined);
</script>

{#if geom}
  <BaseEdge
    path={geom.path}
    {markerEnd}
    label={label as string | undefined}
    labelX={geom.labelX}
    labelY={geom.labelY}
    labelStyle="white-space: pre-line; text-align: center;"
    style={dash ? `stroke-dasharray: ${dash}` : undefined}
  />
{/if}
