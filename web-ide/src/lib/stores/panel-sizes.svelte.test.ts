import { afterEach, describe, expect, it, vi } from "vitest";

import {
  PANEL_MAX,
  PANEL_MIN,
  PROBLEMS_MAX,
  PROBLEMS_MIN,
  panelSizes,
} from "./panel-sizes.svelte.js";

afterEach(() => localStorage.clear());

describe("bounds constants", () => {
  it("exposes the panel and problems-dock ranges", () => {
    expect([PANEL_MIN, PANEL_MAX]).toEqual([180, 520]);
    expect([PROBLEMS_MIN, PROBLEMS_MAX]).toEqual([80, 600]);
  });
});

describe("setters clamp, round and persist", () => {
  it("clamps the explorer width into range and persists it", () => {
    panelSizes.setExplorerW(50);
    expect(panelSizes.explorerW).toBe(180);
    expect(localStorage.getItem("pds-explorer-w")).toBe("180");
    panelSizes.setExplorerW(9999);
    expect(panelSizes.explorerW).toBe(520);
    panelSizes.setExplorerW(300.7);
    expect(panelSizes.explorerW).toBe(301);
  });

  it("clamps the structure width", () => {
    panelSizes.setStructureW(10000);
    expect(panelSizes.structureW).toBe(520);
  });

  it("clamps the problems-dock height to its own range", () => {
    panelSizes.setProblemsH(10);
    expect(panelSizes.problemsH).toBe(80);
    panelSizes.setProblemsH(700);
    expect(panelSizes.problemsH).toBe(600);
  });

  it("resets to defaults", () => {
    panelSizes.setExplorerW(500);
    panelSizes.resetExplorer();
    expect(panelSizes.explorerW).toBe(248);
    panelSizes.resetStructure();
    expect(panelSizes.structureW).toBe(268);
    panelSizes.resetProblems();
    expect(panelSizes.problemsH).toBe(200);
  });
});

describe("load (fresh module instance)", () => {
  it("clamps a stored value above the cap and rejects non-positive ones", async () => {
    localStorage.setItem("pds-problems-h", "700");
    localStorage.setItem("pds-explorer-w", "0"); // → default, not 0
    vi.resetModules();
    const mod = await import("./panel-sizes.svelte.js");
    expect(mod.panelSizes.problemsH).toBe(600); // clampH cap, not the 520 panel cap
    expect(mod.panelSizes.explorerW).toBe(248); // default fallback
  });
});
