// Ephemeral UI state — a reactive rune store.
//
// Panel/modal visibility, the doc reading width (persisted), the recents list,
// the canvas popovers, and the name-prompt / confirm dialogs. The handlers that
// open these (showCanvasInfo, the dialog `run` callbacks, refreshRecents) stay in
// the view; this store just owns the state.

import type { CanvasInfo, CanvasUsages, ConfirmDialog, Dialog } from "$lib/core/types.js";
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
  settingsOpen = $state(false);
  mdHelpOpen = $state(false);
  // The Markdown reading width (narrow | wide | full), persisted across sessions.
  docWidth = $state(readDocWidth());
  // Doc-build progress + the example-vs-folder modal.
  building = $state(false);
  buildNotice = $state(false);
  // Recent projects (folder handles), for the launcher.
  recents = $state<Recent[]>([]);
  // Canvas pointer popovers: hover info / find-usages.
  canvasInfo = $state<CanvasInfo | null>(null);
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
