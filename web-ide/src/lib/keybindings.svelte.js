// User-customisable editor keyboard shortcuts. A fixed command catalogue with
// default chords, plus per-command overrides persisted to localStorage. Values
// are CodeMirror key strings (e.g. "Mod-/", "Shift-F12", "Ctrl-Space");
// Editor.svelte maps each command id to its run function and feeds the effective
// bindings into a keymap compartment, reconfigured whenever `version` changes.

const STORAGE_KEY = "pds.keybindings";

// One entry per customisable command. `key` is the default chord; `group`
// buckets the rows in the settings UI. Array order is the display order.
export const COMMANDS = [
  { id: "triggerAutocomplete", label: "Trigger autocomplete", key: "Ctrl-Space", group: "Editing" },
  { id: "acceptCompletion", label: "Accept completion", key: "Tab", group: "Editing" },
  { id: "toggleComment", label: "Toggle line comment", key: "Mod-/", group: "Editing" },
  { id: "duplicateLine", label: "Duplicate line down", key: "Mod-d", group: "Editing" },
  { id: "formatDocument", label: "Reformat file", key: "Mod-Alt-l", group: "Editing" },
  { id: "saveDocument", label: "Save file", key: "Mod-s", group: "Editing" },
  { id: "openSearch", label: "Find in file", key: "Mod-f", group: "Editing" },
  { id: "goToDefinition", label: "Go to definition", key: "F12", group: "Navigation" },
  { id: "findUsages", label: "Find usages", key: "Shift-F12", group: "Navigation" },
  { id: "openSettings", label: "Keyboard shortcuts…", key: "Mod-,", group: "General" },
];

const DEFAULTS = Object.fromEntries(COMMANDS.map((c) => [c.id, c.key]));
const PROFILE_KEY = "pds.keymap-profile";

// Preset keymaps mirroring the major IDEs, mapping our command set onto each
// IDE's conventions. A profile sets the baseline scheme; per-command overrides
// (recorded in Settings) layer on top and win. `default` uses DEFAULTS.
export const PROFILES = [
  { id: "default", label: "PseudoScript", bindings: {} },
  {
    id: "vscode",
    label: "VS Code",
    bindings: {
      triggerAutocomplete: "Ctrl-Space",
      acceptCompletion: "Tab",
      toggleComment: "Mod-/",
      duplicateLine: "Shift-Alt-ArrowDown",
      formatDocument: "Shift-Alt-f",
      saveDocument: "Mod-s",
      openSearch: "Mod-f",
      goToDefinition: "F12",
      findUsages: "Shift-F12",
      openSettings: "Mod-k Mod-s",
    },
  },
  {
    id: "intellij",
    label: "IntelliJ IDEA",
    bindings: {
      triggerAutocomplete: "Ctrl-Space",
      acceptCompletion: "Tab",
      toggleComment: "Mod-/",
      duplicateLine: "Mod-d",
      formatDocument: "Mod-Alt-l",
      saveDocument: "Mod-s",
      openSearch: "Mod-f",
      goToDefinition: "Mod-b",
      findUsages: "Alt-F7",
      openSettings: "Mod-,",
    },
  },
];

// Load overrides, keeping only known ids with non-empty string chords (so a
// stale or hand-edited entry can't poison the keymap).
function load() {
  try {
    const obj = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    const clean = {};
    for (const c of COMMANDS) if (typeof obj[c.id] === "string" && obj[c.id]) clean[c.id] = obj[c.id];
    return clean;
  } catch {
    return {};
  }
}

function loadProfile() {
  try {
    const id = localStorage.getItem(PROFILE_KEY);
    return PROFILES.some((p) => p.id === id) ? id : "default";
  } catch {
    return "default";
  }
}

// Reactive state and a change counter the editor watches. Overrides hold only
// genuine customisations (resetting an id deletes its entry); `profile` is the
// active preset's id.
const overrides = $state(load());
let profile = $state(loadProfile());
let version = $state(0);

// The active profile's bindings (empty for `default`).
function profileBindings() {
  return PROFILES.find((p) => p.id === profile)?.bindings ?? {};
}
// The baseline chord for a command under the active profile (before overrides).
function base(id) {
  return profileBindings()[id] ?? DEFAULTS[id];
}

function persist() {
  try {
    if (Object.keys(overrides).length) localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
    else localStorage.removeItem(STORAGE_KEY);
    if (profile !== "default") localStorage.setItem(PROFILE_KEY, profile);
    else localStorage.removeItem(PROFILE_KEY);
  } catch {
    // private mode / quota exceeded — bindings still apply this session.
  }
}

export const keybindings = {
  // Bumps on every change; read it inside an effect to react to rebinds.
  get version() {
    return version;
  },
  // The active profile id.
  get profile() {
    return profile;
  },
  // Effective chord for a command id: a personal override wins over the active
  // profile's binding, which wins over the built-in default.
  keyFor(id) {
    return overrides[id] ?? base(id);
  },
  // The profile baseline a reset returns to (no personal override).
  defaultFor(id) {
    return base(id);
  },
  isCustom(id) {
    return id in overrides;
  },
  // The command id currently bound to `chord`, ignoring `exceptId` — for
  // conflict detection in the settings UI.
  conflict(chord, exceptId) {
    for (const c of COMMANDS) {
      if (c.id !== exceptId && (overrides[c.id] ?? base(c.id)) === chord) return c.id;
    }
    return null;
  },
  setKey(id, chord) {
    if (!chord || chord === base(id)) delete overrides[id];
    else overrides[id] = chord;
    version += 1;
    persist();
  },
  reset(id) {
    delete overrides[id];
    version += 1;
    persist();
  },
  // Switch the baseline preset. Personal overrides are kept and stay layered on
  // top (use resetAll for a pure profile). Overrides that happen to match the
  // new profile's binding are pruned so they don't show as customised.
  setProfile(id) {
    if (!PROFILES.some((p) => p.id === id)) return;
    profile = id;
    for (const k of Object.keys(overrides)) if (overrides[k] === base(k)) delete overrides[k];
    version += 1;
    persist();
  },
  resetAll() {
    for (const k of Object.keys(overrides)) delete overrides[k];
    version += 1;
    persist();
  },
};

// Build a CodeMirror key string from a keydown event. Returns null for a lone
// modifier press, so the recorder waits for the real key.
export function chordFromEvent(e) {
  if (e.key === "Control" || e.key === "Alt" || e.key === "Shift" || e.key === "Meta") return null;
  const parts = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Cmd");
  if (e.shiftKey) parts.push("Shift");
  let key;
  if (e.key === " ") key = "Space";
  else if (e.key.length === 1) key = e.key.toLowerCase();
  else key = e.key; // named keys already match CodeMirror: F12, Tab, Enter, ArrowUp…
  parts.push(key);
  return parts.join("-");
}

const SYMBOLS = {
  Mod: "⌘",
  Cmd: "⌘",
  Meta: "⌘",
  Ctrl: "⌃",
  Alt: "⌥",
  Shift: "⇧",
  Space: "␣",
  Enter: "⏎",
  Escape: "Esc",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

// Pretty-print a chord for display: "Mod-Alt-l" → "⌘ ⌥ L". Multi-stroke
// sequences (space-separated, e.g. "Mod-k Mod-s") render each stroke joined by a
// thin gap: "⌘ K  ⌘ S".
export function formatChord(chord) {
  return chord
    .split(" ")
    .map((stroke) =>
      stroke
        .split("-")
        .map((p) => SYMBOLS[p] ?? (p.length === 1 ? p.toUpperCase() : p))
        .join(" "),
    )
    .join("  ");
}
