import { describe, expect, it } from "vitest";

import { computeDiagnostics } from "./diagnostics.js";
import type { Module, OpenFile } from "./types.js";

const files: OpenFile[] = [
  { fqn: "a", path: "a.pds" },
  { fqn: "b", path: "nested/b.pds" },
];
const modules: Module[] = [
  { fqn: "a", source: "" },
  { fqn: "b", source: "" },
];

describe("computeDiagnostics", () => {
  it("tags each diagnostic with its module FQN and counts errors", () => {
    const check = () => [
      { fqn: "a", diagnostics: [{ severity: "error", message: "boom", start_line: 1, start_col: 1 }] },
      { fqn: "b", diagnostics: [{ severity: "warning", message: "meh", start_line: 2, start_col: 3 }] },
    ];
    const r = computeDiagnostics(modules, files, check as never);
    expect(r.problems.map((p) => p.file)).toEqual(["a", "b"]);
    expect(r.errorCount).toBe(1);
    expect([...r.errorPaths]).toEqual(["a.pds"]); // only the module with an error, by path
  });

  it("returns the empty result when the check throws", () => {
    const r = computeDiagnostics(modules, files, () => {
      throw new Error("wasm");
    });
    expect(r).toEqual({ results: null, problems: [], errorCount: 0, errorPaths: new Set() });
  });

  it("collects no error paths when nothing errors", () => {
    const check = () => [{ fqn: "a", diagnostics: [{ severity: "warning" }] }];
    const r = computeDiagnostics(modules, files, check as never);
    expect(r.errorCount).toBe(0);
    expect(r.errorPaths.size).toBe(0);
  });
});
