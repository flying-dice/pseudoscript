<script lang="ts">
  // A C4 relationship edge that floats: it connects the two cards at the nearest
  // points on their borders (shortest path) instead of fixed top/bottom handles,
  // and routes in the line style the user picked (smooth step / bezier / straight
  // / step). Node geometry comes from the live internal nodes, so the edge stays
  // correct under pan / zoom / fit-view.
  import { BaseEdge, getBezierPath, getSmoothStepPath, getStraightPath, useInternalNode } from "@xyflow/svelte";
  import type { EdgeProps } from "@xyflow/svelte";
  import { getEdgeParams } from "$lib/floating-edge.js";

  let { source, target, markerEnd, label, data }: EdgeProps = $props();

  const sourceNode = useInternalNode(source);
  const targetNode = useInternalNode(target);

  // The path + label anchor, recomputed when either node's geometry changes.
  const geom = $derived.by(() => {
    const s = sourceNode.current;
    const t = targetNode.current;
    // Wait until both nodes are measured — an unmeasured (zero-size) node would
    // make the border-intersection maths divide by zero. The hook re-runs once
    // measurement lands, so the edge appears on the next frame.
    if (!s?.measured.width || !s.measured.height || !t?.measured.width || !t.measured.height) return null;

    const { sx, sy, tx, ty, sourcePos, targetPos } = getEdgeParams(s, t);
    const params = { sourceX: sx, sourceY: sy, targetX: tx, targetY: ty, sourcePosition: sourcePos, targetPosition: targetPos };
    const pathType = (data?.pathType as string) ?? "smoothstep";

    const [path, labelX, labelY] =
      pathType === "straight"
        ? getStraightPath({ sourceX: sx, sourceY: sy, targetX: tx, targetY: ty })
        : pathType === "default"
          ? getBezierPath(params)
          : pathType === "step"
            ? getSmoothStepPath({ ...params, borderRadius: 0 })
            : getSmoothStepPath(params);
    return { path, labelX, labelY };
  });
</script>

{#if geom}
  <BaseEdge path={geom.path} {markerEnd} label={label as string | undefined} labelX={geom.labelX} labelY={geom.labelY} />
{/if}
