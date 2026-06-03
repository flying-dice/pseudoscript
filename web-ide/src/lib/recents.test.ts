import { afterEach, describe, expect, it } from "vitest";

import { forget, getRecents, recordFolder } from "./recents.js";

// recents persists to localStorage (jsdom provides it). The IndexedDB handle
// store is absent under jsdom, so `recordFolder` is called with a null handle —
// the catch keeps the metadata entry regardless.

afterEach(() => localStorage.clear());

describe("recordFolder", () => {
  it("stores the display name and folder dir separately, keyed on the dir", async () => {
    await recordFolder("PseudoScript", "model", null);
    const [entry] = getRecents();
    expect(entry).toMatchObject({ key: "folder:model", kind: "folder", name: "PseudoScript", dir: "model" });
  });

  it("keys on the folder dir, not the display name, so re-record bumps the same entry", async () => {
    await recordFolder("PseudoScript", "model", null);
    await recordFolder("PseudoScript Renamed", "model", null);
    const recents = getRecents();
    expect(recents).toHaveLength(1);
    expect(recents[0]).toMatchObject({ key: "folder:model", name: "PseudoScript Renamed" });
  });

  it("distinguishes two folders whose display names match but dirs differ", async () => {
    await recordFolder("Architecture", "alpha", null);
    await recordFolder("Architecture", "beta", null);
    expect(getRecents().map((r) => r.key).sort()).toEqual(["folder:alpha", "folder:beta"]);
  });

  it("forgets an entry by its dir-derived key", async () => {
    await recordFolder("PseudoScript", "model", null);
    forget("folder:model");
    expect(getRecents()).toHaveLength(0);
  });
});
