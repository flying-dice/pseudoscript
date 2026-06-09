import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { COMMANDS, PROFILES, chordFromEvent, formatChord, keybindings } from "./keybindings.svelte.js";

// The store is a module singleton; reset it (and storage) before each test.
beforeEach(() => {
  localStorage.clear();
  keybindings.setProfile("default");
  keybindings.resetAll();
});
afterEach(() => localStorage.clear());

function key(opts: Partial<KeyboardEventInit> & { key: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", opts);
}

describe("chordFromEvent", () => {
  it("returns null for a lone modifier press", () => {
    for (const k of ["Control", "Alt", "Shift", "Meta"]) {
      expect(chordFromEvent(key({ key: k }))).toBeNull();
    }
  });

  it("orders modifiers Ctrl, Alt, Cmd, Shift then the key", () => {
    expect(chordFromEvent(key({ key: "a", ctrlKey: true, shiftKey: true }))).toBe("Ctrl-Shift-a");
    expect(chordFromEvent(key({ key: "L", metaKey: true, altKey: true }))).toBe("Alt-Cmd-l");
  });

  it("maps space and lowercases single chars; named keys pass through", () => {
    expect(chordFromEvent(key({ key: " " }))).toBe("Space");
    expect(chordFromEvent(key({ key: "F12" }))).toBe("F12");
    expect(chordFromEvent(key({ key: "ArrowUp", altKey: true }))).toBe("Alt-ArrowUp");
  });
});

describe("formatChord", () => {
  it("substitutes symbols and uppercases single chars", () => {
    expect(formatChord("Mod-Alt-l")).toBe("⌘ ⌥ L");
    expect(formatChord("Shift-F12")).toBe("⇧ F12");
  });

  it("joins multi-stroke sequences with a double gap", () => {
    expect(formatChord("Mod-k Mod-s")).toBe("⌘ K  ⌘ S");
  });
});

describe("catalogue", () => {
  it("derives defaults from COMMANDS and every profile binds known ids", () => {
    const ids = new Set(COMMANDS.map((c) => c.id));
    for (const p of PROFILES) {
      for (const id of Object.keys(p.bindings)) expect(ids).toContain(id);
    }
  });
});

describe("keyFor precedence and customisation", () => {
  it("uses the built-in default with no override or profile", () => {
    expect(keybindings.keyFor("duplicateLine")).toBe("Mod-d");
    expect(keybindings.isCustom("duplicateLine")).toBe(false);
  });

  it("lets the active profile change the baseline", () => {
    keybindings.setProfile("vscode");
    expect(keybindings.keyFor("duplicateLine")).toBe("Shift-Alt-ArrowDown");
    expect(keybindings.defaultFor("duplicateLine")).toBe("Shift-Alt-ArrowDown");
    expect(keybindings.isCustom("duplicateLine")).toBe(false);
  });

  it("lets a personal override win over the profile", () => {
    keybindings.setProfile("vscode");
    keybindings.setKey("duplicateLine", "Mod-Alt-d");
    expect(keybindings.keyFor("duplicateLine")).toBe("Mod-Alt-d");
    expect(keybindings.isCustom("duplicateLine")).toBe(true);
  });

  it("setKey to the baseline deletes the override", () => {
    keybindings.setKey("duplicateLine", "Mod-x");
    expect(keybindings.isCustom("duplicateLine")).toBe(true);
    keybindings.setKey("duplicateLine", "Mod-d"); // = default
    expect(keybindings.isCustom("duplicateLine")).toBe(false);
  });

  it("reset and resetAll clear overrides", () => {
    keybindings.setKey("saveDocument", "Mod-x");
    keybindings.reset("saveDocument");
    expect(keybindings.isCustom("saveDocument")).toBe(false);
    keybindings.setKey("openSearch", "Mod-y");
    keybindings.resetAll();
    expect(keybindings.isCustom("openSearch")).toBe(false);
  });
});

describe("conflict", () => {
  it("returns the command holding a chord, excluding the queried id", () => {
    expect(keybindings.conflict("Mod-s", "openSearch")).toBe("saveDocument");
    expect(keybindings.conflict("Mod-s", "saveDocument")).toBeNull();
    expect(keybindings.conflict("Mod-never", "saveDocument")).toBeNull();
  });
});

describe("setProfile pruning", () => {
  it("ignores an unknown profile", () => {
    keybindings.setProfile("nope");
    expect(keybindings.profile).toBe("default");
  });

  it("prunes overrides that match the new profile baseline", () => {
    keybindings.setKey("goToDefinition", "Mod-b"); // IntelliJ's default
    expect(keybindings.isCustom("goToDefinition")).toBe(true);
    keybindings.setProfile("intellij");
    expect(keybindings.isCustom("goToDefinition")).toBe(false);
  });

  it("bumps version on every change", () => {
    const v = keybindings.version;
    keybindings.setKey("saveDocument", "Mod-x");
    expect(keybindings.version).toBeGreaterThan(v);
  });
});

describe("persistence (fresh module load)", () => {
  it("keeps only known ids with non-empty chords", async () => {
    localStorage.setItem(
      "pds.keybindings",
      JSON.stringify({ saveDocument: "Mod-x", bogus: "z", duplicateLine: "" }),
    );
    vi.resetModules();
    const mod = await import("./keybindings.svelte.js");
    expect(mod.keybindings.keyFor("saveDocument")).toBe("Mod-x");
    expect(mod.keybindings.isCustom("duplicateLine")).toBe(false); // empty dropped
    expect(mod.keybindings.keyFor("openSearch")).toBe("Mod-f"); // unknown ignored
  });

  it("restores a valid persisted profile", async () => {
    localStorage.setItem("pds.keymap-profile", "vscode");
    vi.resetModules();
    const mod = await import("./keybindings.svelte.js");
    expect(mod.keybindings.profile).toBe("vscode");
  });

  it("falls back to default for an unknown persisted profile", async () => {
    localStorage.setItem("pds.keymap-profile", "atom");
    vi.resetModules();
    const mod = await import("./keybindings.svelte.js");
    expect(mod.keybindings.profile).toBe("default");
  });
});
