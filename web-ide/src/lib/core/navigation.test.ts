import { describe, expect, it } from "vitest";

import {
  canBack,
  canForward,
  HISTORY_CAP,
  type NavState,
  originLoc,
  recordLocation,
  samePosition,
  stepBack,
  stepForward,
} from "./navigation.js";
import type { Loc } from "./types.js";

const at = (fileFqn: string, line: number, col = 1, label?: string): Loc => ({ fileFqn, line, col, label });
const empty: NavState = { history: [], index: -1 };

describe("navigation history", () => {
  it("records the first location at index 0", () => {
    const s = recordLocation(empty, at("m", 1));
    expect(s).toEqual({ history: [at("m", 1)], index: 0 });
  });

  it("collapses a repeat of the current location, refreshing its label", () => {
    let s = recordLocation(empty, at("m", 1, 1, "old"));
    s = recordLocation(s, at("m", 1, 1, "new"));
    expect(s.history).toEqual([at("m", 1, 1, "new")]);
    expect(s.index).toBe(0);
  });

  it("truncates the forward tail when recording after a back step", () => {
    let s = recordLocation(empty, at("m", 1));
    s = recordLocation(s, at("m", 2));
    s = recordLocation(s, at("m", 3));
    s = stepBack(s)!.state; // index → 1 (at line 2)
    s = recordLocation(s, at("m", 9)); // drops line 3
    expect(s.history.map((l) => l.line)).toEqual([1, 2, 9]);
    expect(s.index).toBe(2);
  });

  it("caps the trail at HISTORY_CAP, dropping the oldest", () => {
    let s = empty;
    for (let i = 1; i <= HISTORY_CAP + 5; i++) s = recordLocation(s, at("m", i));
    expect(s.history).toHaveLength(HISTORY_CAP);
    expect(s.history[0].line).toBe(6); // first 5 fell off
    expect(s.index).toBe(HISTORY_CAP - 1);
  });

  it("gates back/forward at the boundaries", () => {
    expect(canBack(empty)).toBe(false);
    expect(canForward(empty)).toBe(false);
    let s = recordLocation(empty, at("m", 1));
    s = recordLocation(s, at("m", 2));
    expect(canBack(s)).toBe(true);
    expect(canForward(s)).toBe(false); // at the tip
    const back = stepBack(s)!;
    expect(back.loc.line).toBe(1);
    expect(canForward(back.state)).toBe(true);
    expect(stepForward(back.state)!.loc.line).toBe(2);
    expect(stepBack(empty)).toBeNull();
    expect(stepForward(empty)).toBeNull();
  });

  it("builds an origin label from the FQN leaf and line", () => {
    expect(originLoc("pkg::Mod", 7, 3)).toEqual({ fileFqn: "pkg::Mod", line: 7, col: 3, label: "Mod:7" });
  });

  it("compares positions by file/line/col, ignoring the label", () => {
    expect(samePosition(at("m", 1, 2, "a"), at("m", 1, 2, "b"))).toBe(true);
    expect(samePosition(at("m", 1, 2), at("m", 1, 3))).toBe(false);
    expect(samePosition(undefined, at("m", 1))).toBe(false);
  });

  it("treats a code entry and a canvas entry at the same position as distinct", () => {
    const code = at("m", 7, 3);
    const canvas: Loc = { fileFqn: "m", line: 7, col: 3, view: "canvas", fqn: "m::Node" };
    expect(samePosition(code, canvas)).toBe(false);
    let s = recordLocation(empty, code);
    s = recordLocation(s, canvas);
    expect(s.history).toHaveLength(2);
    expect(s.index).toBe(1);
  });

  it("treats a space (3D) entry as distinct from canvas/code, and steps to it", () => {
    const canvas: Loc = { fileFqn: "m", line: 7, col: 3, view: "canvas", fqn: "m::Node" };
    const space: Loc = { fileFqn: "m", line: 7, col: 3, view: "space", fqn: "m::Node" };
    expect(samePosition(canvas, space)).toBe(false);
    let s = recordLocation(empty, canvas);
    s = recordLocation(s, space);
    expect(s.history).toHaveLength(2);
    expect(stepBack(s)?.loc.view).toBe("canvas");
  });

  it("collapses a repeat of the same canvas scope", () => {
    const canvas: Loc = { fileFqn: "m", line: 7, col: 3, view: "canvas", fqn: "m::Node", label: "old" };
    let s = recordLocation(empty, canvas);
    s = recordLocation(s, { ...canvas, label: "new" });
    expect(s.history).toEqual([{ ...canvas, label: "new" }]);
    expect(s.index).toBe(0);
  });

  it("distinguishes two canvas scopes at the same position by fqn", () => {
    const a: Loc = { fileFqn: "m", line: 0, col: 0, view: "canvas", fqn: "m::A" };
    const b: Loc = { fileFqn: "m", line: 0, col: 0, view: "canvas", fqn: "m::B" };
    expect(samePosition(a, b)).toBe(false);
    let s = recordLocation(empty, a);
    s = recordLocation(s, b);
    expect(s.history.map((l) => l.fqn)).toEqual(["m::A", "m::B"]);
  });

  it("preserves the view through a back step", () => {
    let s = recordLocation(empty, { fileFqn: "m", line: 1, col: 1, view: "canvas", fqn: "m::A" });
    s = recordLocation(s, at("m", 2));
    const back = stepBack(s)!;
    expect(back.loc.view).toBe("canvas");
    expect(back.loc.fqn).toBe("m::A");
  });
});
