// Ephemeral UI state — a reactive rune store.
//
// Panel/modal visibility, the doc reading width (persisted), the recents list,
// the canvas usages popover, and the name-prompt / confirm dialogs. The handlers
// that open these (showCanvasUsages, the dialog `run` callbacks, refreshRecents)
// stay in the view; this store just owns the state.

import type { CanvasUsages, ConfirmDialog, Dialog } from "$lib/core/types.js";
import type { Recent } from "$lib/recents.js";

function readDocWidth(): string {
  try {
    return localStorage.getItem("pds-doc-width") || "narrow";
  } catch {
    return "narrow";
  }
}

class UiStore {
  // The launcher / settings / markdown-help panels.
  projectOpen = $state(false);
  // The New-project dialog (template picker), opened from the launcher.
  newProjectOpen = $state(false);
  settingsOpen = $state(false);
  mdHelpOpen = $state(false);
  // Tool-window islands: the left-hand Explorer (file tree) and right-hand
  // Structure panel are open by default; the bottom Problems dock starts closed.
  explorerOpen = $state(true);
  structureOpen = $state(true);
  problemsOpen = $state(false);
  // The ⌘K command palette (go-to file / symbol).
  commandOpen = $state(false);
  // The Markdown reading width (narrow | wide | full), persisted across sessions.
  docWidth = $state(readDocWidth());
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
}

export const ui = new UiStore();
