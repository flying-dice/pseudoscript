// Reconstructs the Graphviz `dot` source equivalent to what the
// `pseudoscript-emit` layout engine feeds its `pseudoscript-dot` port for a C4
// view: a box per card (sized as the engine sized it), nested `subgraph
// cluster_*` for the boundary frames, and one edge per relationship. Pasting it
// into the Graphviz playground (https://dreampuf.github.io/GraphvizOnline or
// https://magjac.com/graphviz-visual-editor) shows how *real* `dot` lays the
// same graph out — a quick check of our port against the reference.

type Rect = { x: number; y: number; w: number; h: number };
type LaidOutNode = { fqn: string; label: string; rect: Rect };
type LaidOutEdge = { from: string; to: string; labels?: string[] };
type BoundaryFrame = { fqn: string; title: string; rect: Rect };
export type C4LayoutLike = {
  nodes: LaidOutNode[];
  edges: LaidOutEdge[];
  boundaries?: BoundaryFrame[];
};

const PT_PER_IN = 72;
const EPS = 0.5;
const q = (s: string) => `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
const area = (r: Rect) => r.w * r.h;
const encloses = (outer: Rect, inner: Rect) =>
  outer.x <= inner.x + EPS &&
  outer.y <= inner.y + EPS &&
  outer.x + outer.w >= inner.x + inner.w - EPS &&
  outer.y + outer.h >= inner.y + inner.h - EPS;

/** The smallest frame whose rectangle strictly encloses `r`, excluding `selfFqn`.
 *  Uses rect-enclosure, not centre-containment, so concentric nested frames nest
 *  correctly (an outer frame's centre also falls inside its inner child). */
function enclosingFrame(frames: BoundaryFrame[], r: Rect, selfFqn?: string): string | null {
  let best: BoundaryFrame | null = null;
  for (const f of frames) {
    if (f.fqn === selfFqn || !encloses(f.rect, r) || area(f.rect) <= area(r)) continue;
    if (!best || area(f.rect) < area(best.rect)) best = f;
  }
  return best ? best.fqn : null;
}

/** Build a `dot` source for a positioned C4 layout. `lr` selects `rankdir=LR`. */
export function buildC4Dot(layout: C4LayoutLike, lr = false): string {
  const frames = layout.boundaries ?? [];
  // Each card / frame's direct parent frame: the smallest frame enclosing it.
  const cardParent = new Map<string, string | null>(
    layout.nodes.map((n) => [n.fqn, enclosingFrame(frames, n.rect)]),
  );
  const frameParent = new Map<string, string | null>(
    frames.map((f) => [f.fqn, enclosingFrame(frames, f.rect, f.fqn)]),
  );

  const childCards = (frame: string | null) =>
    layout.nodes.filter((n) => (cardParent.get(n.fqn) ?? null) === frame);
  const childFrames = (frame: string | null) =>
    frames.filter((f) => (frameParent.get(f.fqn) ?? null) === frame);

  const emitFrame = (f: BoundaryFrame, depth: number): string => {
    const pad = "  ".repeat(depth);
    let s = `${pad}subgraph ${q("cluster_" + f.fqn)} {\n${pad}  label=${q(f.title)};\n`;
    for (const cf of childFrames(f.fqn)) s += emitFrame(cf, depth + 1);
    for (const c of childCards(f.fqn)) {
      s += `${pad}  ${q(c.fqn)} [label=${q(c.label)}, width=${(c.rect.w / PT_PER_IN).toFixed(3)}, height=${(c.rect.h / PT_PER_IN).toFixed(3)}];\n`;
    }
    return s + `${pad}}\n`;
  };

  let dot = `digraph {\n  rankdir=${lr ? "LR" : "TB"};\n  node [shape=box, fixedsize=true];\n`;
  // Free cards (outside every frame) declared at the top level.
  for (const c of childCards(null)) {
    dot += `  ${q(c.fqn)} [label=${q(c.label)}, width=${(c.rect.w / PT_PER_IN).toFixed(3)}, height=${(c.rect.h / PT_PER_IN).toFixed(3)}];\n`;
  }
  for (const f of childFrames(null)) dot += emitFrame(f, 1);
  for (const e of layout.edges) {
    const labels = (e.labels ?? []).join("\n");
    dot += `  ${q(e.from)} -> ${q(e.to)}${labels ? ` [label=${q(labels)}]` : ""};\n`;
  }
  return dot + "}\n";
}
