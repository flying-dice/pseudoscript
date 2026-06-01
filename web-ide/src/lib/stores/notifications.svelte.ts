// Toast / notification state — a reactive rune store.
//
// Owns the toast stack and the transient flash message plus their timers (the
// impure edge). The view reads `notifications.notes` / `.toast` and calls
// `notify` / `dismiss` / `flash`. No business logic lives here — see core/* for
// that; this is just owned reactive state.

import type { Note, NoteKind } from "$lib/core/types.js";

class NotificationsStore {
  // The stacked toasts (success / error / info), top-right.
  notes = $state<Note[]>([]);
  // The transient flash message (centre), or null.
  toast = $state<string | null>(null);

  #seq = 0;
  #toastTimer: ReturnType<typeof setTimeout> | undefined;

  /** Push a toast; auto-dismisses (errors linger longer). */
  notify(kind: NoteKind, title: string, body = ""): void {
    const id = (this.#seq += 1);
    this.notes = [...this.notes, { id, kind, title, body }];
    setTimeout(() => this.dismiss(id), kind === "error" ? 9000 : 6000);
  }

  /** Remove a toast by id. */
  dismiss(id: string | number): void {
    this.notes = this.notes.filter((n) => n.id !== id);
  }

  /** Show a short-lived flash message (2.4s). */
  flash(message: string): void {
    this.toast = message;
    clearTimeout(this.#toastTimer);
    this.#toastTimer = setTimeout(() => (this.toast = null), 2400);
  }
}

export const notifications = new NotificationsStore();
