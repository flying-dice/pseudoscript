// Navigation history — a reactive rune store over the pure core/navigation reducer.
//
// Owns the trail + cursor; the impure application of a location (opening files,
// jumping the editor) stays in the view, which calls `back()`/`forward()` and
// applies the returned Loc. All trail math lives in core/navigation.

import * as nav from "$lib/core/navigation.js";
import type { Loc } from "$lib/core/types.js";

class NavigationStore {
  history = $state<Loc[]>([]);
  index = $state(-1);

  get #state(): nav.NavState {
    return { history: this.history, index: this.index };
  }
  #set(s: nav.NavState): void {
    this.history = s.history;
    this.index = s.index;
  }

  get canBack(): boolean {
    return nav.canBack(this.#state);
  }
  get canForward(): boolean {
    return nav.canForward(this.#state);
  }

  /** The current cursor location, or undefined. */
  current(): Loc | undefined {
    return this.history[this.index];
  }

  /** Record a visited location (forward-tail truncation, repeat collapse, cap). */
  record(loc: Loc): void {
    this.#set(nav.recordLocation(this.#state, loc));
  }

  /** Record `loc` unless the cursor already sits there (origin-recording). */
  recordIfMoved(loc: Loc): void {
    if (!nav.samePosition(this.current(), loc)) this.record(loc);
  }

  /** Step back, returning the location to apply, or null at the boundary. */
  back(): Loc | null {
    const step = nav.stepBack(this.#state);
    if (!step) return null;
    this.#set(step.state);
    return step.loc;
  }

  /** Step forward, returning the location to apply, or null at the boundary. */
  forward(): Loc | null {
    const step = nav.stepForward(this.#state);
    if (!step) return null;
    this.#set(step.state);
    return step.loc;
  }

  /** Clear the trail (on workspace mount). */
  reset(): void {
    this.history = [];
    this.index = -1;
  }
}

export const navigation = new NavigationStore();
