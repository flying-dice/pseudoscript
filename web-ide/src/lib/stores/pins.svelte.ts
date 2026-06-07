// Manual grid placements for the open workspace — a reactive rune store.
//
// Holds the parsed `pds.layout` document, the "unlocked" (drag-to-pin) flag, and
// the file handle when the workspace is folder-backed (samples/shares are
// session-only, handle stays null). The view owns persistence (it has the
// workspace root + writeFile); this store just owns the state and pure edits.

import {
  clearPin,
  clearView,
  emptyLayoutDoc,
  parseLayoutDoc,
  setPin,
  setPins,
  type LayoutDoc,
  type Pin,
} from "$lib/core/pins.js";

class PinStore {
  /** The whole-project pins, keyed by view. */
  doc = $state<LayoutDoc>(emptyLayoutDoc());
  /** Drag-to-pin editing enabled (session-only; reset per workspace). */
  unlocked = $state(false);
  /** The `pds.layout` handle for a folder-backed workspace, else null. */
  handle = $state<FileSystemFileHandle | null>(null);

  /** Load a workspace's pins (from `pds.layout` text) and its handle, locked. */
  load(text: string, handle: FileSystemFileHandle | null): void {
    this.doc = parseLayoutDoc(text);
    this.handle = handle;
    this.unlocked = false;
  }

  /** Forget all pins (a fresh in-memory workspace with no layout file). */
  reset(): void {
    this.doc = emptyLayoutDoc();
    this.handle = null;
    this.unlocked = false;
  }

  /** Pin a node to a cell in `key`'s view (reassigns `doc` for reactivity). */
  pin(key: string, pin: Pin): void {
    this.doc = setPin(this.doc, key, pin);
  }

  /** Freeze a view's whole arrangement (pin every node at its current cell), so
   *  dragging one box leaves the rest exactly where they are. */
  freeze(key: string, list: Pin[]): void {
    this.doc = setPins(this.doc, key, list);
  }

  /** Un-pin a node in `key`'s view. */
  unpin(key: string, fqn: string): void {
    this.doc = clearPin(this.doc, key, fqn);
  }

  /** Reset one view to the auto-layout. */
  clear(key: string): void {
    this.doc = clearView(this.doc, key);
  }
}

export const pins = new PinStore();
