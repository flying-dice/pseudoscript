// Ephemeral UI state — a reactive rune store.
//
// Panel/modal visibility, the doc reading width (persisted), the recents list,
// the canvas usages popover, and the name-prompt / confirm dialogs. The handlers
// that open these (showCanvasUsages, the dialog `run` callbacks, refreshRecents)
// stay in the view; this store just owns the state.

import { DEFAULT_LAYOUT_TWEAKS } from "$lib/core/types.js";
import type { CanvasUsages, ConfirmDialog, Dialog, LayoutTweaks } from "$lib/core/types.js";
import type { Recent } from "$lib/recents.js";

function readDocWidth(): string {
  try {
    return localStorage.getItem("pds-doc-width") || "narrow";
  } catch {
    return "narrow";
  }
}

function readPerfHud(): boolean {
  try {
    const v = localStorage.getItem("pds-perf-hud");
    // Off unless explicitly enabled (or in dev, where it aids the work).
    return v === null ? import.meta.env.DEV : v === "1";
  } catch {
    return false;
  }
}

function readLayoutTweaks(): LayoutTweaks {
  try {
    const raw = localStorage.getItem("pds-layout");
    // Always a fresh object (never the shared default by reference) and defaults
    // fill any absent field (forward-compatible with older entries).
    const stored = raw ? (JSON.parse(raw) as Partial<LayoutTweaks>) : {};
    return { ...DEFAULT_LAYOUT_TWEAKS, ...stored };
  } catch {
    return { ...DEFAULT_LAYOUT_TWEAKS };
  }
}

class UiStore {
  // The launcher / settings / markdown-help panels.
  projectOpen = $state(false);
  // The New-project dialog (template picker), opened from the launcher.
  newProjectOpen = $state(false);
  settingsOpen = $state(false);
  mdHelpOpen = $state(false);
  // The in-product language reference (Help menu).
  referenceOpen = $state(false);
  // The status-bar frame-rate / heap readout — a developer aid, off by default
  // in production; toggled from the View menu and persisted.
  perfHud = $state(readPerfHud());
  // Tool-window islands: the left-hand Explorer (file tree) and right-hand
  // Structure panel are open by default; the bottom Problems dock starts closed.
  explorerOpen = $state(true);
  structureOpen = $state(true);
  problemsOpen = $state(false);
  // The ⌘K command palette (go-to file / symbol).
  commandOpen = $state(false);
  // The Markdown reading width (narrow | wide | full), persisted across sessions.
  docWidth = $state(readDocWidth());
  // The C4 layout tweaks (the canvas "Layout" control), one config applied to
  // every diagram and persisted across sessions.
  layoutTweaks = $state<LayoutTweaks>(readLayoutTweaks());
  // Doc-build progress + the example-vs-folder modal.
  building = $state(false);
  buildNotice = $state(false);
  // Recent projects (folder handles), for the launcher.
  recents = $state<Recent[]>([]);
  // Canvas pointer popover: the find-usages reference list.
  canvasUsages = $state<CanvasUsages | null>(null);
  // The FileTree name-prompt and destructive-confirm dialogs.
  dialog = $state<Dialog | null>(null);
  confirmDialog = $state<ConfirmDialog | null>(null);

  /** Set and persist the doc reading width. */
  setDocWidth(w: string): void {
    this.docWidth = w;
    try {
      localStorage.setItem("pds-doc-width", w);
    } catch {
      /* storage unavailable — session-only */
    }
  }

  /** Toggle and persist the performance HUD. */
  togglePerfHud(): void {
    this.perfHud = !this.perfHud;
    try {
      localStorage.setItem("pds-perf-hud", this.perfHud ? "1" : "0");
    } catch {
      /* storage unavailable — session-only */
    }
  }

  /** Set and persist the layout tweaks (applied to every diagram). */
  setLayoutTweaks(tweaks: LayoutTweaks): void {
    this.layoutTweaks = tweaks;
    try {
      localStorage.setItem("pds-layout", JSON.stringify(tweaks));
    } catch {
      /* storage unavailable — session-only */
    }
  }
}

export const ui = new UiStore();
