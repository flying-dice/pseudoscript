import { describe, expect, it } from "vitest";
import { collapseSequence, type Info, type Scene } from "./sequence.js";

// fqn -> { kind, parent, summary }: a system holding a container holding a
// component, the structural ancestry the collapse and parent-path walk follow.
const info: Info = {
  "m::Shop": { kind: "system", parent: null, summary: "The shop." },
  "m::Api": { kind: "container", parent: "m::Shop", summary: "HTTP surface." },
  "m::Validator": { kind: "component", parent: "m::Api", summary: "Checks orders." },
};

const scene: Scene = {
  participants: [
    { fqn: "client", kind: "person" },
    { fqn: "m::Validator", kind: "component", summary: "Checks orders.", parent_path: "Shop::Api" },
  ],
  items: [{ Message: { from: "client", to: "m::Validator", kind: "call", label: "checkout" } }],
};

describe("collapseSequence", () => {
  it("collapses a component to its container, carrying the ancestor's summary and ancestry", () => {
    const out = collapseSequence(scene, "container", info)!;
    const api = out.participants.find((p) => p.fqn === "m::Api")!;
    expect(api.kind).toBe("container");
    expect(api.summary).toBe("HTTP surface.");
    expect(api.parent_path).toBe("Shop"); // enclosing system name only
  });

  it("leaves a person lifeline with no parent path", () => {
    const out = collapseSequence(scene, "container", info)!;
    const client = out.participants.find((p) => p.fqn === "client")!;
    expect(client.parent_path ?? null).toBeNull();
  });

  it("is the identity at component depth (keeps the projected summary/ancestry)", () => {
    expect(collapseSequence(scene, "component", info)).toBe(scene);
  });
});
