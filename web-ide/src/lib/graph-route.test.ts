import { describe, expect, it } from "vitest";

import { ancestors, routeOf, type ParentOf } from "./graph-route";

// A two-system tree:
//   SysA → ContA → CompA1, CompA2
//   SysB → ContB → CompB1
const tree: ParentOf = new Map<string, string | null>([
  ["SysA", null],
  ["ContA", "SysA"],
  ["CompA1", "ContA"],
  ["CompA2", "ContA"],
  ["SysB", null],
  ["ContB", "SysB"],
  ["CompB1", "ContB"],
]);

describe("ancestors", () => {
  it("walks to the root", () => {
    expect(ancestors("CompA1", tree)).toEqual(["CompA1", "ContA", "SysA"]);
  });

  it("a root is its own only ancestor", () => {
    expect(ancestors("SysA", tree)).toEqual(["SysA"]);
  });

  it("stops at the first absent ancestor", () => {
    const orphan: ParentOf = new Map([["X", "Missing"]]);
    expect(ancestors("X", orphan)).toEqual(["X"]);
  });
});

describe("routeOf", () => {
  it("siblings meet at their shared container; the bridge straddles it", () => {
    const r = routeOf("CompA1", "CompA2", tree);
    expect(r.path).toEqual(["CompA1", "ContA", "CompA2"]);
    expect(r.bridge).toEqual(["CompA1", "CompA2"]);
  });

  it("a node and its own ancestor share a line — no bridge", () => {
    const r = routeOf("CompA1", "ContA", tree);
    expect(r.path).toEqual(["CompA1", "ContA"]);
    expect(r.bridge).toBeNull();
  });

  it("across systems, joins the two roots and bridges them", () => {
    const r = routeOf("CompA1", "CompB1", tree);
    expect(r.path).toEqual(["CompA1", "ContA", "SysA", "SysB", "ContB", "CompB1"]);
    expect(r.bridge).toEqual(["SysA", "SysB"]);
  });

  it("is directional in the path but routes through the same gateways", () => {
    const fwd = routeOf("CompA1", "CompB1", tree);
    const rev = routeOf("CompB1", "CompA1", tree);
    expect(rev.path).toEqual([...fwd.path].reverse());
  });
});
