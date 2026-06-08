import { afterEach, describe, expect, it, vi } from "vitest";

import { THEME_OPTIONS } from "./theme.svelte.js";

// Each test imports a fresh module instance (the store caches its MediaQueryList),
// with matchMedia stubbed and a controllable `matches` flag.
async function freshTheme(dark = false) {
  document.head.innerHTML = '<meta name="theme-color" content="">';
  document.documentElement.removeAttribute("data-theme");
  const state = { matches: dark };
  const change: { cb: ((e: unknown) => void) | null } = { cb: null };
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => ({
      get matches() {
        return state.matches;
      },
      media: "(prefers-color-scheme: dark)",
      addEventListener: (_: string, cb: (e: unknown) => void) => {
        change.cb = cb;
      },
      removeEventListener: () => {},
    })),
  );
  vi.resetModules();
  const mod = await import("./theme.svelte.js");
  return { theme: mod.theme, state, change };
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

it("exposes the three preferences", () => {
  expect(THEME_OPTIONS).toEqual(["system", "light", "dark"]);
});

describe("set", () => {
  it("pins light: resolved, data-theme, meta colour and storage", async () => {
    const { theme } = await freshTheme();
    theme.set("light");
    expect(theme.resolved).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(document.querySelector('meta[name="theme-color"]')?.getAttribute("content")).toBe("#e7eaef");
    expect(localStorage.getItem("pds-theme")).toBe("light");
  });

  it("pins dark with its meta colour", async () => {
    const { theme } = await freshTheme();
    theme.set("dark");
    expect(theme.resolved).toBe("dark");
    expect(document.querySelector('meta[name="theme-color"]')?.getAttribute("content")).toBe("#0a0b0e");
    expect(localStorage.getItem("pds-theme")).toBe("dark");
  });

  it("removes storage when set back to system", async () => {
    const { theme } = await freshTheme();
    theme.set("dark");
    theme.set("system");
    expect(localStorage.getItem("pds-theme")).toBeNull();
  });

  it("ignores an invalid preference", async () => {
    const { theme } = await freshTheme();
    theme.set("light");
    theme.set("neon" as never);
    expect(theme.resolved).toBe("light");
  });
});

describe("system resolution", () => {
  it("resolves dark/light from the OS preference", async () => {
    expect((await freshTheme(true)).theme.resolved).toBe("dark");
    expect((await freshTheme(false)).theme.resolved).toBe("light");
  });

  it("re-resolves on a live OS change while preference is system", async () => {
    const { theme, state, change } = await freshTheme(false);
    theme.init();
    expect(theme.resolved).toBe("light");
    state.matches = true;
    change.cb?.({});
    expect(theme.resolved).toBe("dark");
  });

  it("ignores OS changes once a theme is pinned", async () => {
    const { theme, state, change } = await freshTheme(false);
    theme.init();
    theme.set("light");
    state.matches = true;
    change.cb?.({});
    expect(theme.resolved).toBe("light");
  });
});

describe("loadPref", () => {
  it("loads a valid stored preference", async () => {
    localStorage.setItem("pds-theme", "dark");
    expect((await freshTheme()).theme.pref).toBe("dark");
  });

  it("falls back to system for a garbage stored value", async () => {
    localStorage.setItem("pds-theme", "rainbow");
    expect((await freshTheme()).theme.pref).toBe("system");
  });
});
