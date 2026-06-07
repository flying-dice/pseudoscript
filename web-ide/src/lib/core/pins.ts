// Manual grid placements ("pins") for the C4 canvas, persisted to `pds.layout`.
//
// When grid placement is active and a diagram is unlocked, dragging a node pins it
// to a grid cell. Pins are `(row, col)` cells (stable across grid resize), keyed
// per diagram view by `(c4view, of)`. The engine fixes pinned nodes and searches
// only the rest. Pure data + transforms — no Svelte, no I/O.

/** A node fixed to a grid cell, by FQN. */
export type Pin = { fqn: string; row: number; col: number };

/** The grid geometry the engine emits (canvas pixels): cell `(r,c)` is centred at
 *  `origin + (c·cell_w, r·cell_h)`. */
export type GridGeom = {
  cols: number;
  rows: number;
  cell_w: number;
  cell_h: number;
  origin: { x: number; y: number };
};

/** The cell a pixel point `(x, y)` falls in, clamped into the grid — the inverse
 *  of the cell-centre formula, shared by drag-snap and freeze. */
export function cellAt(g: GridGeom, x: number, y: number): { row: number; col: number } {
  const clamp = (v: number, hi: number): number => Math.max(0, Math.min(hi, v));
  return {
    col: clamp(Math.round((x - g.origin.x) / g.cell_w), g.cols - 1),
    row: clamp(Math.round((y - g.origin.y) / g.cell_h), g.rows - 1),
  };
}

/** The whole-project layout document: pins for every view, keyed by {@link viewKey}. */
export type LayoutDoc = { version: 1; views: Record<string, Pin[]> };

/** An empty document. */
export function emptyLayoutDoc(): LayoutDoc {
  return { version: 1, views: {} };
}

/** The stable key for a diagram view: its kind and target FQN (`""` for the
 *  whole-model context overview). Matches the engine's scene identity. */
export function viewKey(view: string, of: string | null | undefined): string {
  return `${view}|${of ?? ""}`;
}

/** The pins for one view, or `[]`. */
export function getPins(doc: LayoutDoc, key: string): Pin[] {
  return doc.views[key] ?? [];
}

/** Pin `fqn` at `(row, col)` in `view` without disturbing other boxes. If the
 *  target cell is occupied and the moving node already had a cell, the two swap
 *  (the occupant takes the vacated cell) — a predictable local trade, never a
 *  reshuffle. If the moving node was un-pinned, the occupant is evicted (rejoins
 *  the free search). Returns a new document. */
export function setPin(doc: LayoutDoc, key: string, pin: Pin): LayoutDoc {
  const cur = getPins(doc, key);
  const from = cur.find((p) => p.fqn === pin.fqn); // the moving node's old cell
  const occupant = cur.find((p) => p.row === pin.row && p.col === pin.col && p.fqn !== pin.fqn);
  let next = cur.filter((p) => p.fqn !== pin.fqn);
  if (occupant && from) {
    // Swap: the occupant slides into the cell the moving node just left.
    next = next.map((p) => (p.fqn === occupant.fqn ? { ...p, row: from.row, col: from.col } : p));
  } else if (occupant) {
    next = next.filter((p) => p.fqn !== occupant.fqn);
  }
  return { ...doc, views: { ...doc.views, [key]: [...next, pin] } };
}

/** Replace a view's pins wholesale (used to freeze the current arrangement when
 *  the grid is unlocked). Returns a new document. */
export function setPins(doc: LayoutDoc, key: string, list: Pin[]): LayoutDoc {
  return { ...doc, views: { ...doc.views, [key]: [...list] } };
}

/** Remove the pin for `fqn` in `view` (un-pin). Returns a new document. */
export function clearPin(doc: LayoutDoc, key: string, fqn: string): LayoutDoc {
  return { ...doc, views: { ...doc.views, [key]: getPins(doc, key).filter((p) => p.fqn !== fqn) } };
}

/** Drop every pin for one view (reset it to the auto-layout). Returns a new document. */
export function clearView(doc: LayoutDoc, key: string): LayoutDoc {
  const views = { ...doc.views };
  delete views[key];
  return { ...doc, views };
}

/** Whether any view holds a pin. */
export function hasPins(doc: LayoutDoc): boolean {
  return Object.values(doc.views).some((ps) => ps.length > 0);
}

/** Parse `pds.layout` text, tolerating malformed input (→ an empty document). */
export function parseLayoutDoc(text: string): LayoutDoc {
  try {
    const raw = JSON.parse(text) as unknown;
    if (!raw || typeof raw !== "object") return emptyLayoutDoc();
    const views = (raw as { views?: unknown }).views;
    if (!views || typeof views !== "object") return emptyLayoutDoc();
    const out: Record<string, Pin[]> = {};
    for (const [key, list] of Object.entries(views as Record<string, unknown>)) {
      if (!Array.isArray(list)) continue;
      out[key] = list
        .filter(
          (p): p is Pin =>
            !!p &&
            typeof (p as Pin).fqn === "string" &&
            Number.isFinite((p as Pin).row) &&
            Number.isFinite((p as Pin).col),
        )
        .map((p) => ({ fqn: p.fqn, row: Math.trunc(p.row), col: Math.trunc(p.col) }));
    }
    return { version: 1, views: out };
  } catch {
    return emptyLayoutDoc();
  }
}

/** Serialise a document to pretty JSON for `pds.layout`. */
export function serializeLayoutDoc(doc: LayoutDoc): string {
  return `${JSON.stringify(doc, null, 2)}\n`;
}
