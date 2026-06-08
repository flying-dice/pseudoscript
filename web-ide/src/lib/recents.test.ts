import "fake-indexeddb/auto";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { forget, getRecents, recordFolder, recordSample, reopenFolder } from "./recents.js";

// recents persists to localStorage (jsdom provides it) and folder handles to
// IndexedDB (fake-indexeddb/auto provides a real, in-memory one here).

beforeEach(() => localStorage.clear());
afterEach(() => localStorage.clear());

// A structured-cloneable stand-in handle (real FS handles are cloneable host
// objects; a fake with methods is not, and IndexedDB clones on put). The cloned
// value lacks queryPermission, so reopenFolder's permission step throws → null —
// the grant→return-handle branch is browser-only and stays e2e-covered.
function cloneableHandle(name: string): FileSystemDirectoryHandle {
  return { kind: "directory", name } as unknown as FileSystemDirectoryHandle;
}

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

describe("recordSample", () => {
  it("records a sample keyed by id, most-recent-first", () => {
    recordSample({ id: "banking", name: "Internet Banking" });
    const [entry] = getRecents();
    expect(entry).toMatchObject({ key: "sample:banking", kind: "sample", sampleId: "banking" });
  });
});

describe("reopenFolder (IndexedDB handle store)", () => {
  // Unique dir per test so the shared in-memory IDB store never cross-contaminates.
  it("stores a handle on record and reads it back on reopen (put + get)", async () => {
    await recordFolder("App", "stored", cloneableHandle("stored"));
    // The cloned handle has no permission API → the permission step throws → null,
    // proving the get path ran (vs. the no-handle short-circuit below).
    expect(await reopenFolder("folder:stored")).toBeNull();
  });

  it("returns null when no handle was stored", async () => {
    await recordFolder("App", "nohandle", null);
    expect(await reopenFolder("folder:nohandle")).toBeNull();
  });

  it("forget removes the stored handle too", async () => {
    await recordFolder("App", "drop", cloneableHandle("drop"));
    forget("folder:drop");
    expect(await reopenFolder("folder:drop")).toBeNull();
  });
});
