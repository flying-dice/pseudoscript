// Floating-edge geometry: anchor an edge at the point where the straight line
// between two node centres crosses each node's border, rather than at a fixed
// handle. This gives the shortest visible connection and lets edges leave a card
// from whichever side faces the other node. Ported from Svelte Flow's floating-
// edges example, adapted to absolute node positions (parent offsets included).

import { Position, type InternalNode } from "@xyflow/svelte";

type Rect = { x: number; y: number; w: number; h: number };

function rect(node: InternalNode): Rect {
  return {
    x: node.internals.positionAbsolute.x,
    y: node.internals.positionAbsolute.y,
    w: node.measured.width ?? 0,
    h: node.measured.height ?? 0,
  };
}

// Where the segment from `node`'s centre toward `other`'s centre meets node's
// border. Projects the centre-to-centre direction into the rectangle's
// normalised diagonal space, clamps it to the unit diamond (so it lands on an
// edge), then maps back — Svelte Flow's floating-edges intersection.
function intersection(node: Rect, other: Rect): { x: number; y: number } {
  const halfW = node.w / 2;
  const halfH = node.h / 2;
  const cx = node.x + halfW;
  const cy = node.y + halfH;
  const ox = other.x + other.w / 2;
  const oy = other.y + other.h / 2;

  // Direction to the other centre, rotated into the rectangle's diagonal axes.
  const diagA = (ox - cx) / (2 * halfW) - (oy - cy) / (2 * halfH);
  const diagB = (ox - cx) / (2 * halfW) + (oy - cy) / (2 * halfH);
  const clamp = 1 / (Math.abs(diagA) + Math.abs(diagB) || 1);
  const unitA = clamp * diagA;
  const unitB = clamp * diagB;
  return { x: halfW * (unitA + unitB) + cx, y: halfH * (-unitA + unitB) + cy };
}

// Which border the intersection sits on — drives the path's entry/exit direction.
function side(node: Rect, p: { x: number; y: number }): Position {
  const nx = Math.round(node.x);
  const ny = Math.round(node.y);
  const px = Math.round(p.x);
  const py = Math.round(p.y);
  if (px <= nx + 1) return Position.Left;
  if (px >= nx + node.w - 1) return Position.Right;
  if (py <= ny + 1) return Position.Top;
  return Position.Bottom;
}

// The floating endpoints + their borders for a source→target pair.
export function getEdgeParams(source: InternalNode, target: InternalNode) {
  const s = rect(source);
  const t = rect(target);
  const sp = intersection(s, t);
  const tp = intersection(t, s);
  return {
    sx: sp.x,
    sy: sp.y,
    tx: tp.x,
    ty: tp.y,
    sourcePos: side(s, sp),
    targetPos: side(t, tp),
  };
}
