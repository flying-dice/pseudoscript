// Save-lifecycle state — a reactive rune store.
//
// Owns the persisted on-disk baseline (the dirty comparison) and the active
// file's save-state cue. The debounce/FS write methods and `reloadExternalChanges`
// stay in the view (they touch FS handles, timers, and the manifest re-resolve);
// the pure baseline/dirty math lives in core/dirty.

class SaveStore {
  // The baseline: text last read from / written to disk, keyed like the live
  // buffers (FQN for modules, path for docs/manifest). A buffer differing from
  // its baseline is dirty; handle-less samples never enter this map.
  persisted = $state<Record<string, string>>({});
  // The active file's save lifecycle, for the status cue.
  saveState = $state<"idle" | "saving" | "saved" | "error">("idle");
}

export const saveStore = new SaveStore();
