// User-customisable editor keyboard shortcuts. A fixed command catalogue with
// default chords, plus per-command overrides persisted to localStorage. Values
// are CodeMirror key strings (e.g. "Mod-/", "Shift-F12", "Ctrl-Space");
// Editor.svelte maps each command id to its run function and feeds the effective
// bindings into a keymap compartment, reconfigured whenever `version` changes.

const STORAGE_KEY = "pds.keybindings";

// A single customisable command in the catalogue.
export interface Command {
  id: string;
  label: string;
  key: string;
  group: string;
}

// A preset keymap: a baseline scheme mapping command ids to chords.
export interface Profile {
  id: string;
  label: string;
  bindings: Record<string, string>;
}

// One entry per customisable command. `key` is the default chord; `group`
// buckets the rows in the settings UI. Array order is the display order.
export const COMMANDS: Command[] = [
  { id: "triggerAutocomplete", label: "Trigger autocomplete", key: "Ctrl-Space", group: "Editing" },
  { id: "acceptCompletion", label: "Accept completion", key: "Tab", group: "Editing" },
  { id: "toggleComment", label: "Toggle line comment", key: "Mod-/", group: "Editing" },
  { id: "duplicateLine", label: "Duplicate line down", key: "Mod-d", group: "Editing" },
  { id: "formatDocument", label: "Reformat file", key: "Mod-Alt-l", group: "Editing" },
  { id: "saveDocument", label: "Save file", key: "Mod-s", group: "Editing" },
  { id: "openSearch", label: "Find in file", key: "Mod-f", group: "Editing" },
  { id: "goToDefinition", label: "Go to definition", key: "F12", group: "Navigation" },
  { id: "findUsages", label: "Find usages", key: "Shift-F12", group: "Navigation" },
  { id: "openSettings", label: "Settings…", key: "Mod-,", group: "General" },
];

const DEFAULTS: Record<string, string> = Object.fromEntries(COMMANDS.map((c) => [c.id, c.key]));
const PROFILE_KEY = "pds.keymap-profile";

// Preset keymaps mirroring the major IDEs, mapping our command set onto each
// IDE's conventions. A profile sets the baseline scheme; per-command overrides
// (recorded in Settings) layer on top and win. `default` uses DEFAULTS.
export const PROFILES: Profile[] = [
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
function load(): Record<string, string> {
  try {
    const obj: Record<string, unknown> = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    const clean: Record<string, string> = {};
    for (const c of COMMANDS) {
      const v = obj[c.id];
      if (typeof v === "string" && v) clean[c.id] = v;
    }
    return clean;
  } catch {
    return {};
  }
}

function loadProfile(): string {
  try {
    const id = localStorage.getItem(PROFILE_KEY);
    return id !== null && PROFILES.some((p) => p.id === id) ? id : "default";
  } catch {
    return "default";
  }
}

// Reactive state and a change counter the editor watches. Overrides hold only
// genuine customisations (resetting an id deletes its entry); `profile` is the
// active preset's id.
const overrides: Record<string, string> = $state(load());
let profile: string = $state(loadProfile());
let version: number = $state(0);

// The active profile's bindings (empty for `default`).
function profileBindings(): Record<string, string> {
  return PROFILES.find((p) => p.id === profile)?.bindings ?? {};
}
// The baseline chord for a command under the active profile (before overrides).
function base(id: string): string {
  return profileBindings()[id] ?? DEFAULTS[id];
}

function persist(): void {
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
  keyFor(id: string): string {
    return overrides[id] ?? base(id);
  },
  // The profile baseline a reset returns to (no personal override).
  defaultFor(id: string): string {
    return base(id);
  },
  isCustom(id: string): boolean {
    return id in overrides;
  },
  // The command id currently bound to `chord`, ignoring `exceptId` — for
  // conflict detection in the settings UI.
  conflict(chord: string, exceptId: string): string | null {
    for (const c of COMMANDS) {
      if (c.id !== exceptId && (overrides[c.id] ?? base(c.id)) === chord) return c.id;
    }
    return null;
  },
  setKey(id: string, chord: string): void {
    if (!chord || chord === base(id)) delete overrides[id];
    else overrides[id] = chord;
    version += 1;
    persist();
  },
  reset(id: string): void {
    delete overrides[id];
    version += 1;
    persist();
  },
  // Switch the baseline preset. Personal overrides are kept and stay layered on
  // top (use resetAll for a pure profile). Overrides that happen to match the
  // new profile's binding are pruned so they don't show as customised.
  setProfile(id: string): void {
    if (!PROFILES.some((p) => p.id === id)) return;
    profile = id;
    for (const k of Object.keys(overrides)) if (overrides[k] === base(k)) delete overrides[k];
    version += 1;
    persist();
  },
  resetAll(): void {
    for (const k of Object.keys(overrides)) delete overrides[k];
    version += 1;
    persist();
  },
};

// Build a CodeMirror key string from a keydown event. Returns null for a lone
// modifier press, so the recorder waits for the real key.
export function chordFromEvent(e: KeyboardEvent): string | null {
  if (e.key === "Control" || e.key === "Alt" || e.key === "Shift" || e.key === "Meta") return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Cmd");
  if (e.shiftKey) parts.push("Shift");
  let key: string;
  if (e.key === " ") key = "Space";
  else if (e.key.length === 1) key = e.key.toLowerCase();
  else key = e.key; // named keys already match CodeMirror: F12, Tab, Enter, ArrowUp…
  parts.push(key);
  return parts.join("-");
}

const SYMBOLS: Record<string, string> = {
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
export function formatChord(chord: string): string {
  return chord
    .split(" ")
    .map((stroke: string) =>
      stroke
        .split("-")
        .map((p: string) => SYMBOLS[p] ?? (p.length === 1 ? p.toUpperCase() : p))
        .join(" "),
    )
    .join("  ");
}
