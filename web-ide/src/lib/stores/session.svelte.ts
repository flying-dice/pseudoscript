// A reactive marker for when the IdeSession's held workspace last (re)mounted.
//
// The wasm `IdeSession` holds the workspace state, but it is a plain object — not
// a reactive store. View state derived from it (the structure outline, workspace
// diagnostics) therefore can't track the session directly. The mount runs in an
// `$effect` (after render), so a derived that reads the session on first render
// sees the *pre-mount* (empty) state and, with nothing reactive to invalidate it,
// never recomputes.
//
// `sessionMount.seq` closes that gap: the structural mount bumps it once the
// session actually holds the new modules, and held-state deriveds read it so they
// recompute against the mounted session. (Per-keystroke edits don't bump it —
// they flow through `setIdeSource` and the source change already invalidates the
// deriveds via `allModules`.)
class SessionMount {
  seq = $state(0);

  /** Mark a completed (re)mount, invalidating held-state deriveds. */
  bump(): void {
    this.seq += 1;
  }
}

export const sessionMount = new SessionMount();
