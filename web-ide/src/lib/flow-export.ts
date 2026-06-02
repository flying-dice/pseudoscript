// Export a live svelte-flow canvas to a downloadable image. Captures the
// `.svelte-flow__viewport` DOM with html-to-image, so the output matches the
// canvas pixel-for-pixel (kind colours, cards, floating edges, lifelines). The
// viewport is re-framed to fit *every* node — independent of the current pan /
// zoom — using xyflow's pure bounds helpers, so the export is the whole diagram,
// not a screenshot of the visible region.

import { getViewportForBounds, type Node, type Rect } from "@xyflow/svelte";
import { toPng, toSvg } from "html-to-image";

export type ExportFormat = "png" | "svg";

// Frame bounds: keep the export legible without rendering an unbounded canvas.
const PADDING = 0.1; // fraction of the frame left as margin around the nodes
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 2;
const MAX_EDGE = 4096; // cap the long edge (px) to bound memory
const PNG_PIXEL_RATIO = 2; // crisper raster on hi-dpi / when scaled up

// The bounding rect of every node, in flow coordinates. Boundary views nest
// children under a parent with positions relative to it, so resolve each node's
// absolute position by summing its parent chain before measuring. (The standalone
// `getNodesBounds` can't resolve sub-flows without a nodeLookup; this can.)
function nodesBounds(nodes: Node[]): Rect {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const n of nodes) {
    let x = n.position?.x ?? 0;
    let y = n.position?.y ?? 0;
    for (let p = n.parentId ? byId.get(n.parentId) : undefined; p; p = p.parentId ? byId.get(p.parentId) : undefined) {
      x += p.position?.x ?? 0;
      y += p.position?.y ?? 0;
    }
    const w = n.measured?.width ?? n.width ?? 0;
    const h = n.measured?.height ?? n.height ?? 0;
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x + w);
    maxY = Math.max(maxY, y + h);
  }
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

// Whole-diagram pixel size from the node bounds, capped at MAX_EDGE while
// preserving aspect ratio.
function frameSize(width: number, height: number): { width: number; height: number } {
  const scale = Math.min(1, MAX_EDGE / Math.max(width, height));
  return { width: Math.ceil(width * scale), height: Math.ceil(height * scale) };
}

/**
 * Render the flow inside `container` to a PNG/SVG and trigger a browser download.
 * `nodes` are the component's measured Svelte Flow nodes; `background` is a CSS
 * colour painted behind the diagram. Rejects when the diagram has no nodes.
 */
export async function downloadDiagram(
  container: HTMLElement,
  nodes: Node[],
  opts: { format: ExportFormat; filename: string; background: string },
): Promise<void> {
  if (nodes.length === 0) throw new Error("nothing to export");

  const viewport = container.querySelector<HTMLElement>(".svelte-flow__viewport");
  if (!viewport) throw new Error("diagram not ready");

  const bounds = nodesBounds(nodes);
  const { width, height } = frameSize(bounds.width, bounds.height);
  const vp = getViewportForBounds(bounds, width, height, MIN_ZOOM, MAX_ZOOM, PADDING);

  const style = {
    width: `${width}px`,
    height: `${height}px`,
    transform: `translate(${vp.x}px, ${vp.y}px) scale(${vp.zoom})`,
  };
  const common = { backgroundColor: opts.background, width, height, style };

  const dataUrl =
    opts.format === "png"
      ? await toPng(viewport, { ...common, pixelRatio: PNG_PIXEL_RATIO })
      : await toSvg(viewport, common);

  const a = document.createElement("a");
  a.href = dataUrl;
  a.download = `${opts.filename}.${opts.format}`;
  a.click();
}
