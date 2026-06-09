import { describe, expect, it } from "vitest";

import {
  type GridGeom,
  type LayoutDoc,
  cellAt,
  clearPin,
  clearView,
  emptyLayoutDoc,
  getPins,
  hasPins,
  parseLayoutDoc,
  serializeLayoutDoc,
  setPin,
  viewKey,
} from "./pins.js";

const geom: GridGeom = { cols: 4, rows: 3, cell_w: 100, cell_h: 50, origin: { x: 10, y: 20 }, pad: 0 };

describe("cellAt", () => {
  it("inverts the cell-centre formula", () => {
    expect(cellAt(geom, 110, 70)).toEqual({ col: 1, row: 1 });
    expect(cellAt(geom, 10, 20)).toEqual({ col: 0, row: 0 });
  });

  it("rounds to the nearest cell centre", () => {
    expect(cellAt(geom, 155, 70)).toEqual({ col: 1, row: 1 }); // 1.45 → 1
    expect(cellAt(geom, 165, 70)).toEqual({ col: 2, row: 1 }); // 1.55 → 2
  });

  it("clamps below 0 and above the last cell", () => {
    expect(cellAt(geom, -500, -500)).toEqual({ col: 0, row: 0 });
    expect(cellAt(geom, 9999, 9999)).toEqual({ col: 3, row: 2 });
  });

  it("subtracts the frame pad so a pixel maps to its pin cell", () => {
    // 8×8 grid, 2-cell frame. Raw cell (3, 3) → pin cell (1, 1).
    const g: GridGeom = { cols: 8, rows: 8, cell_w: 100, cell_h: 100, origin: { x: 0, y: 0 }, pad: 2 };
    expect(cellAt(g, 300, 300)).toEqual({ col: 1, row: 1 });
    // The top-left frame clamps to pin cell 0 (you can't pin into the reserved band).
    expect(cellAt(g, 0, 0)).toEqual({ col: 0, row: 0 });
    // The bottom-right frame clamps to the last pin cell (cols - 1 - 2·pad = 3).
    expect(cellAt(g, 9999, 9999)).toEqual({ col: 3, row: 3 });
  });
});

describe("viewKey", () => {
  it("joins view and target fqn", () => {
    expect(viewKey("c4", "a::B")).toBe("c4|a::B");
  });

  it("uses an empty target for null/undefined (whole-model overview)", () => {
    expect(viewKey("c4", null)).toBe("c4|");
    expect(viewKey("c4", undefined)).toBe("c4|");
  });
});

describe("getPins", () => {
  it("returns the view's pins or an empty array", () => {
    const doc: LayoutDoc = { version: 1, views: { k: [{ fqn: "a", row: 0, col: 0 }] } };
    expect(getPins(doc, "k")).toHaveLength(1);
    expect(getPins(doc, "missing")).toEqual([]);
  });
});

describe("setPin", () => {
  const k = "c4|";

  it("appends a new pin to an empty view", () => {
    const next = setPin(emptyLayoutDoc(), k, { fqn: "a", row: 1, col: 2 });
    expect(getPins(next, k)).toEqual([{ fqn: "a", row: 1, col: 2 }]);
  });

  it("re-pins the same node to a free cell with no duplicate", () => {
    let doc = setPin(emptyLayoutDoc(), k, { fqn: "a", row: 0, col: 0 });
    doc = setPin(doc, k, { fqn: "a", row: 2, col: 3 });
    expect(getPins(doc, k)).toEqual([{ fqn: "a", row: 2, col: 3 }]);
  });

  it("swaps when the target is occupied and the mover already had a cell", () => {
    let doc = setPin(emptyLayoutDoc(), k, { fqn: "a", row: 0, col: 0 });
    doc = setPin(doc, k, { fqn: "b", row: 1, col: 1 });
    doc = setPin(doc, k, { fqn: "a", row: 1, col: 1 }); // a moves onto b
    const pins = getPins(doc, k);
    expect(pins).toContainEqual({ fqn: "a", row: 1, col: 1 });
    expect(pins).toContainEqual({ fqn: "b", row: 0, col: 0 }); // b takes a's old cell
  });

  it("evicts the occupant when the mover was un-pinned", () => {
    let doc = setPin(emptyLayoutDoc(), k, { fqn: "b", row: 1, col: 1 });
    doc = setPin(doc, k, { fqn: "a", row: 1, col: 1 }); // a has no prior cell
    expect(getPins(doc, k)).toEqual([{ fqn: "a", row: 1, col: 1 }]);
  });

  it("returns a new document, leaving the input untouched", () => {
    const doc = emptyLayoutDoc();
    const next = setPin(doc, k, { fqn: "a", row: 0, col: 0 });
    expect(next).not.toBe(doc);
    expect(doc.views).toEqual({});
  });
});

describe("clearPin / clearView", () => {
  const k = "c4|";

  it("clears one pin, leaving the rest", () => {
    let doc = setPin(emptyLayoutDoc(), k, { fqn: "a", row: 0, col: 0 });
    doc = setPin(doc, k, { fqn: "b", row: 1, col: 1 });
    expect(getPins(clearPin(doc, k, "a"), k)).toEqual([{ fqn: "b", row: 1, col: 1 }]);
  });

  it("clearPin on an absent fqn is a no-op", () => {
    const doc = setPin(emptyLayoutDoc(), k, { fqn: "a", row: 0, col: 0 });
    expect(getPins(clearPin(doc, k, "z"), k)).toEqual([{ fqn: "a", row: 0, col: 0 }]);
  });

  it("clearView deletes the key entirely", () => {
    const doc = setPin(emptyLayoutDoc(), k, { fqn: "a", row: 0, col: 0 });
    const next = clearView(doc, k);
    expect(Object.keys(next.views)).not.toContain(k);
  });
});

describe("hasPins", () => {
  it("is false for empty or all-empty views, true when any view holds a pin", () => {
    expect(hasPins(emptyLayoutDoc())).toBe(false);
    expect(hasPins({ version: 1, views: { k: [] } })).toBe(false);
    expect(hasPins({ version: 1, views: { k: [{ fqn: "a", row: 0, col: 0 }] } })).toBe(true);
  });
});

describe("parseLayoutDoc", () => {
  it("round-trips valid JSON", () => {
    const doc: LayoutDoc = { version: 1, views: { k: [{ fqn: "a", row: 1, col: 2 }] } };
    expect(parseLayoutDoc(JSON.stringify(doc))).toEqual(doc);
  });

  it("returns an empty doc for non-JSON, null, string or array roots", () => {
    for (const text of ["not json", "null", '"a string"', "[1,2,3]"]) {
      expect(parseLayoutDoc(text)).toEqual(emptyLayoutDoc());
    }
  });

  it("returns an empty doc when views is missing or not an object", () => {
    expect(parseLayoutDoc('{"version":1}')).toEqual(emptyLayoutDoc());
    expect(parseLayoutDoc('{"views":42}')).toEqual(emptyLayoutDoc());
  });

  it("skips entries that aren't arrays", () => {
    const out = parseLayoutDoc('{"views":{"k":42,"ok":[]}}');
    expect(out.views).toEqual({ ok: [] });
  });

  it("filters pins missing fqn or with non-finite coords", () => {
    const text = JSON.stringify({
      views: { k: [{ row: 0, col: 0 }, { fqn: "a", row: NaN, col: 0 }, { fqn: "b", row: 1, col: 1 }] },
    });
    expect(parseLayoutDoc(text).views.k).toEqual([{ fqn: "b", row: 1, col: 1 }]);
  });

  it("truncates fractional row/col", () => {
    const text = JSON.stringify({ views: { k: [{ fqn: "a", row: 2.9, col: -1.9 }] } });
    expect(parseLayoutDoc(text).views.k).toEqual([{ fqn: "a", row: 2, col: -1 }]);
  });
});

describe("serializeLayoutDoc", () => {
  it("pretty-prints with a trailing newline and round-trips", () => {
    const doc: LayoutDoc = { version: 1, views: { k: [{ fqn: "a", row: 0, col: 0 }] } };
    const text = serializeLayoutDoc(doc);
    expect(text.endsWith("\n")).toBe(true);
    expect(text).toContain("  ");
    expect(parseLayoutDoc(text)).toEqual(doc);
  });
});
