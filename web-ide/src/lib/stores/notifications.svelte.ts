// Toast / notification state — a reactive rune store.
//
// Owns the toast stack plus its dismiss timers (the impure edge). The view reads
// `notifications.notes` and calls `notify` / `dismiss`. No business logic lives
// here — see core/* for that; this is just owned reactive state.

import type { Note, NoteKind } from "$lib/core/types.js";

// Auto-dismiss tiers by message weight (ms): errors linger longest, then toasts
// carrying body detail, then bare title-only confirmations.
const TTL_ERROR = 9000;
const TTL_WITH_BODY = 6000;
const TTL_TITLE_ONLY = 3000;

class NotificationsStore {
  // The stacked toasts (success / error / info), bottom-right.
  notes = $state<Note[]>([]);

  #seq = 0;

  /** Push a toast; auto-dismisses on a tier set by message weight. */
  notify(kind: NoteKind, title: string, body = ""): void {
    const id = (this.#seq += 1);
    this.notes = [...this.notes, { id, kind, title, body }];
    const ttl = kind === "error" ? TTL_ERROR : body ? TTL_WITH_BODY : TTL_TITLE_ONLY;
    setTimeout(() => this.dismiss(id), ttl);
  }

  /** Remove a toast by id. */
  dismiss(id: string | number): void {
    this.notes = this.notes.filter((n) => n.id !== id);
  }
}

export const notifications = new NotificationsStore();
