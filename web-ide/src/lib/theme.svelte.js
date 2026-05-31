// Theme store: a tri-state preference (system | light | dark), persisted to
// localStorage and applied as `data-theme` on <html>. "system" follows
// `prefers-color-scheme` live. An inline script in app.html applies the resolved
// theme before first paint (no FOUC); this store keeps it in sync at runtime and
// owns the toggle UI's state.

const STORAGE_KEY = "pds-theme";
export const THEME_OPTIONS = ["system", "light", "dark"];

function loadPref() {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return THEME_OPTIONS.includes(v) ? v : "system";
  } catch {
    return "system";
  }
}

function systemPrefersDark() {
  return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches;
}

// The concrete theme ("light" | "dark") for a preference.
function resolve(pref) {
  if (pref === "light" || pref === "dark") return pref;
  return systemPrefersDark() ? "dark" : "light";
}

// Apply the resolved theme to the document + the address-bar theme-color meta.
function apply(resolved) {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", resolved);
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", resolved === "light" ? "#f7f5f0" : "#0a0b0e");
}

let pref = $state(loadPref());
let resolved = $state(typeof window === "undefined" ? "dark" : resolve(loadPref()));

// Re-resolve when "system" and the OS preference changes.
let mql;
function watchSystem() {
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
  get pref() {
    return pref;
  },
  get resolved() {
    return resolved;
  },
  // Apply the current preference (called once on mount to sync runtime state with
  // whatever the inline head script set, and to start watching the OS preference).
  init() {
    watchSystem();
    resolved = resolve(pref);
    apply(resolved);
  },
  set(next) {
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
