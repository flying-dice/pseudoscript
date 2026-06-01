// Theme store: a tri-state preference (system | light | dark), persisted to
// localStorage and applied as `data-theme` on <html>. "system" follows
// `prefers-color-scheme` live. An inline script in app.html applies the resolved
// theme before first paint (no FOUC); this store keeps it in sync at runtime and
// owns the toggle UI's state.

// A user preference: "system" follows the OS, "light"/"dark" pin a theme.
export type ThemePref = "system" | "light" | "dark";
// The concrete theme actually applied to the document.
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "pds-theme";
export const THEME_OPTIONS: readonly ThemePref[] = ["system", "light", "dark"];

function isThemePref(v: string | null): v is ThemePref {
  return v !== null && (THEME_OPTIONS as readonly string[]).includes(v);
}

function loadPref(): ThemePref {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return isThemePref(v) ? v : "system";
  } catch {
    return "system";
  }
}

function systemPrefersDark(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches;
}

// The concrete theme ("light" | "dark") for a preference.
function resolve(pref: ThemePref): ResolvedTheme {
  if (pref === "light" || pref === "dark") return pref;
  return systemPrefersDark() ? "dark" : "light";
}

// Apply the resolved theme to the document + the address-bar theme-color meta.
function apply(resolved: ResolvedTheme): void {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", resolved);
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", resolved === "light" ? "#f7f5f0" : "#0a0b0e");
}

let pref = $state<ThemePref>(loadPref());
let resolved = $state<ResolvedTheme>(typeof window === "undefined" ? "dark" : resolve(loadPref()));

// Re-resolve when "system" and the OS preference changes.
let mql: MediaQueryList | undefined;
function watchSystem(): void {
  if (typeof matchMedia !== "function" || mql) return;
  mql = matchMedia("(prefers-color-scheme: dark)");
  mql.addEventListener?.("change", () => {
    if (pref === "system") {
      resolved = resolve(pref);
      apply(resolved);
    }
  });
}

export const theme = {
  get pref(): ThemePref {
    return pref;
  },
  get resolved(): ResolvedTheme {
    return resolved;
  },
  // Apply the current preference (called once on mount to sync runtime state with
  // whatever the inline head script set, and to start watching the OS preference).
  init(): void {
    watchSystem();
    resolved = resolve(pref);
    apply(resolved);
  },
  set(next: ThemePref): void {
    if (!THEME_OPTIONS.includes(next)) return;
    pref = next;
    resolved = resolve(next);
    apply(resolved);
    try {
      if (next === "system") localStorage.removeItem(STORAGE_KEY);
      else localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // private mode / quota — applies this session only
    }
  },
};
