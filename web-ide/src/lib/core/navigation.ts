// Navigation history — a pure reducer over a visited-location trail.
//
// The view owns the impure half (opening files, jumping the editor, reading the
// caret); this module owns the trail math: recording with browser-history
// semantics (forward-tail truncation, repeat collapse, a bound), and stepping
// back/forward. No Svelte, no DOM.

import type { Loc } from "./types.js";

// The history trail and the cursor into it. `index === -1` means empty.
export type NavState = { history: Loc[]; index: number };

// The maximum trail length kept; older entries fall off the front.
export const HISTORY_CAP = 50;

/**
 * Whether two locations are the same history entry (label aside): same pane
 * (`view`, absent ⇒ `"code"`), same code position, and same node scope (`fqn`).
 * Including `view`/`fqn` keeps a code entry and a canvas entry at the same
 * declaration position distinct, while a repeat of one scope still collapses.
 */
export function samePosition(a: Loc | undefined, b: Loc): boolean {
  return (
    !!a &&
    a.fileFqn === b.fileFqn &&
    a.line === b.line &&
    a.col === b.col &&
    (a.view ?? "code") === (b.view ?? "code") &&
    (a.fqn ?? "") === (b.fqn ?? "")
  );
}

/**
 * Record a visited location. Drops any forward tail (a new jump invalidates
 * "forward"), collapses a repeat of the current location (refreshing its label),
 * and caps the trail. Returns the next state; never mutates the input.
 */
export function recordLocation(state: NavState, loc: Loc, cap = HISTORY_CAP): NavState {
  const trail = state.history.slice(0, state.index + 1);
  const last = trail.at(-1);
  let history: Loc[];
  if (samePosition(last, loc)) {
    history = [...trail.slice(0, -1), loc];
  } else {
    history = [...trail, loc].slice(-cap);
  }
  return { history, index: history.length - 1 };
}

/** Whether a back step is available. */
export function canBack(state: NavState): boolean {
  return state.index > 0;
}

/** Whether a forward step is available. */
export function canForward(state: NavState): boolean {
  return state.index >= 0 && state.index < state.history.length - 1;
}

/** Step back one entry, returning the new state and the location to apply, or null. */
export function stepBack(state: NavState): { state: NavState; loc: Loc } | null {
  if (!canBack(state)) return null;
  const index = state.index - 1;
  return { state: { ...state, index }, loc: state.history[index] };
}

/** Step forward one entry, returning the new state and the location to apply, or null. */
export function stepForward(state: NavState): { state: NavState; loc: Loc } | null {
  if (!canForward(state)) return null;
  const index = state.index + 1;
  return { state: { ...state, index }, loc: state.history[index] };
}

/**
 * The origin location to record before a jump, so Back returns to where the caret
 * was. `fqn` is the open module's FQN; the label is its leaf name and line.
 */
export function originLoc(fqn: string, line: number, col: number): Loc {
  return { fileFqn: fqn, line, col, label: `${fqn.split("::").at(-1)}:${line}` };
}
