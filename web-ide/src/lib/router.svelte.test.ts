import { describe, expect, it } from "vitest";

import { type Route, emptyRoute, parseHash, serializeRoute } from "./router.svelte.js";

const roundTrips = (r: Route) => expect(parseHash(serializeRoute(r))).toEqual(r);

describe("parseHash", () => {
  it("parses a folder base with view and caret", () => {
    expect(parseHash("#/f.model/code?f=orders&l=12&c=4")).toEqual({
      base: { kind: "f", value: "model" },
      view: "code",
      file: "orders",
      line: 12,
      col: 4,
    });
  });

  it("decodes a percent-encoded folder name and node fqn", () => {
    expect(parseHash("#/f.my%20app/canvas?n=orders%3A%3APlace")).toEqual({
      base: { kind: "f", value: "my app" },
      view: "canvas",
      node: "orders::Place",
    });
  });

  it("parses an embedded workspace base", () => {
    expect(parseHash("#/w.H4sIAAAA/space?n=shared")).toEqual({
      base: { kind: "w", value: "H4sIAAAA" },
      view: "space",
      node: "shared",
    });
  });

  it("defaults the view to code when absent or unknown", () => {
    expect(parseHash("#/f.model").view).toBe("code");
    expect(parseHash("#/f.model/bogus").view).toBe("code");
  });

  it("coerces numeric params and drops non-numbers", () => {
    expect(parseHash("#/f.m/code?l=7&c=0").line).toBe(7);
    expect(parseHash("#/f.m/code?l=7&c=0").col).toBe(0);
    expect(parseHash("#/f.m/code?l=nope").line).toBeUndefined();
  });

  it("yields a kind-null base for a bare or legacy hash (caller's fallback)", () => {
    expect(parseHash("").base.kind).toBeNull();
    expect(parseHash("#").base.kind).toBeNull();
    expect(parseHash("#w=H4sIAAAA").base.kind).toBeNull(); // legacy share link
  });
});

describe("serializeRoute", () => {
  it("encodes the folder name and :: fqns", () => {
    const hash = serializeRoute({
      base: { kind: "f", value: "my app" },
      view: "canvas",
      node: "orders::Place",
    });
    expect(hash).toBe("#/f.my%20app/canvas?n=orders%3A%3APlace");
  });

  it("leaves a base64url workspace payload unescaped", () => {
    expect(serializeRoute({ base: { kind: "w", value: "H4s_-A" }, view: "code" })).toBe("#/w.H4s_-A/code");
  });

  it("serializes a kind-null base to a bare hash", () => {
    expect(serializeRoute(emptyRoute())).toBe("#");
  });
});

describe("round-trips", () => {
  it("preserves every field across each base kind and view", () => {
    roundTrips({ base: { kind: "f", value: "model" }, view: "code", file: "orders", line: 3, col: 9 });
    roundTrips({ base: { kind: "f", value: "a b/c" }, view: "canvas", node: "x::Y", depth: "container" });
    roundTrips({ base: { kind: "w", value: "AAAA__--" }, view: "space", node: "shared" });
  });
});
