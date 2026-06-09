import { describe, expect, it } from "vitest";

import { SAMPLES, sampleSeed } from "./samples.js";

// Asserts structural invariants over the live, glob-discovered catalogue rather
// than specific sample names/counts, which churn as examples are added.

describe("SAMPLES catalogue", () => {
  it("discovers at least one sample, each well-formed", () => {
    expect(SAMPLES.length).toBeGreaterThan(0);
    for (const s of SAMPLES) {
      expect(s.id).toBeTruthy();
      expect(s.name).toBeTruthy();
      expect(s.files.length).toBeGreaterThan(0);
      expect(s.moduleCount).toBe(s.files.length);
    }
  });

  it("sorts by order, then name", () => {
    for (let i = 1; i < SAMPLES.length; i++) {
      const a = SAMPLES[i - 1];
      const b = SAMPLES[i];
      expect(a.order < b.order || (a.order === b.order && a.name.localeCompare(b.name) <= 0)).toBe(true);
    }
  });

  it("sorts files within a sample by fqn", () => {
    for (const s of SAMPLES) {
      const fqns = s.files.map((f) => f.fqn);
      expect(fqns).toEqual([...fqns].sort((a, b) => a.localeCompare(b)));
    }
  });
});

describe("sampleSeed", () => {
  it("returns null for an unknown id", () => {
    expect(sampleSeed("does-not-exist")).toBeNull();
  });

  it("seeds every module, the manifest when present, and doc pages", () => {
    const sample = SAMPLES[0];
    const out = sampleSeed(sample.id);
    expect(out).not.toBeNull();
    const paths = out!.seed.map((f) => f.path);
    for (const f of sample.files) expect(paths).toContain(f.path);
    if (sample.manifestToml) expect(paths).toContain("pds.toml");
    for (const docPath of Object.keys(sample.docs)) expect(paths).toContain(docPath);
    expect(out!.landing).toBe(sample.landing);
  });
});
