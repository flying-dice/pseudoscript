import { afterEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_LAYOUT_TWEAKS } from "$lib/core/types.js";
import { ui } from "./ui.svelte.js";

afterEach(() => localStorage.clear());

describe("UiStore defaults", () => {
  it("opens the explorer and structure, closes the problems dock", () => {
    expect(ui.explorerOpen).toBe(true);
    expect(ui.structureOpen).toBe(true);
    expect(ui.problemsOpen).toBe(false);
    expect(ui.commandOpen).toBe(false);
  });
});

describe("setDocWidth", () => {
  it("updates and persists the reading width", () => {
    ui.setDocWidth("wide");
    expect(ui.docWidth).toBe("wide");
    expect(localStorage.getItem("pds-doc-width")).toBe("wide");
    ui.setDocWidth("narrow");
  });
});

describe("setLayoutTweaks", () => {
  it("updates and persists the layout tweaks as JSON", () => {
    const tweaks = { ...DEFAULT_LAYOUT_TWEAKS };
    ui.setLayoutTweaks(tweaks);
    expect(ui.layoutTweaks).toEqual(tweaks);
    expect(JSON.parse(localStorage.getItem("pds-layout")!)).toEqual(tweaks);
  });
});

describe("readLayoutTweaks (fresh module load)", () => {
  it("defaults the doc width to narrow when unset", async () => {
    vi.resetModules();
    const mod = await import("./ui.svelte.js");
    expect(mod.ui.docWidth).toBe("narrow");
  });

  it("merges defaults over a stored partial, yielding a distinct object", async () => {
    localStorage.setItem("pds-layout", JSON.stringify({ minimizeLongEdges: true }));
    vi.resetModules();
    const mod = await import("./ui.svelte.js");
    expect(mod.ui.layoutTweaks).toMatchObject({ ...DEFAULT_LAYOUT_TWEAKS, minimizeLongEdges: true });
    expect(mod.ui.layoutTweaks).not.toBe(DEFAULT_LAYOUT_TWEAKS);
  });

  it("falls back to fresh defaults on malformed JSON", async () => {
    localStorage.setItem("pds-layout", "{not json");
    vi.resetModules();
    const mod = await import("./ui.svelte.js");
    expect(mod.ui.layoutTweaks).toEqual(DEFAULT_LAYOUT_TWEAKS);
    expect(mod.ui.layoutTweaks).not.toBe(DEFAULT_LAYOUT_TWEAKS);
  });
});
