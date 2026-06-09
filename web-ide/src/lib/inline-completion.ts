// Inline ghost-text completion (model: ide::GhostText). CodeMirror 6 has no
// native inline-suggestion UI — @codemirror/autocomplete only does the dropdown —
// so this is a custom view plugin: an idle debounce gathers a suggestion from the
// host's async source, a widget decoration renders it as greyed text at the
// caret, Tab accepts and Escape dismisses. Any edit or caret move clears it, and
// the in-flight request is aborted on the next keystroke; a source failure is
// reported to the host through `onError` (aborts stay silent) — ghost text must
// never block typing.

import { completionStatus } from "@codemirror/autocomplete";
import { Prec, StateEffect, StateField } from "@codemirror/state";
import type { EditorState, Extension } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, keymap } from "@codemirror/view";
import type { Command, ViewUpdate } from "@codemirror/view";

/** The host's suggestion source: fetch, vet, and pace. */
export interface InlineCompletionSource {
  /** Async suggestion for the caret; resolve "" (or reject) for none. */
  fetch: (doc: string, pos: number, signal: AbortSignal) => Promise<string>;
  /** Vet a suggestion before it shows; returning false drops it. */
  validate?: (doc: string, pos: number, text: string) => boolean;
  /** Idle time after the last keystroke before fetching (default 400ms). */
  debounceMs?: number;
  /** A fetch failure (never an abort) — the host surfaces the reason. */
  onError?: (err: unknown) => void;
  /** The provider answered (the suggestion may still be dropped before it
   * renders) — the host clears any surfaced fetch failure. */
  onAnswered?: () => void;
  /** An answer that never rendered: blank, or rejected by the validator. */
  onDrop?: (reason: "empty" | "invalid") => void;
  /** A suggestion rendered — the host clears any surfaced drop note. */
  onShown?: () => void;
}

type Ghost = { pos: number; text: string };

const setGhost = StateEffect.define<Ghost>();
const clearGhost = StateEffect.define<null>();

class GhostWidget extends WidgetType {
  constructor(readonly text: string) {
    super();
  }
  override eq(other: GhostWidget): boolean {
    return other.text === this.text;
  }
  override toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-ghost-text";
    span.setAttribute("data-testid", "ghost-text");
    span.setAttribute("aria-hidden", "true");
    span.textContent = this.text;
    return span;
  }
}

// The suggestion under display, or null. Any document change or caret move
// invalidates it — acceptance re-inserts the text as a real edit.
const ghostField = StateField.define<Ghost | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setGhost)) return e.value;
      if (e.is(clearGhost)) return null;
    }
    return tr.docChanged || tr.selection ? null : value;
  },
  provide: (field) =>
    EditorView.decorations.from(field, (ghost) =>
      ghost
        ? Decoration.set([
            Decoration.widget({ widget: new GhostWidget(ghost.text), side: 1 }).range(ghost.pos),
          ])
        : Decoration.none,
    ),
});

/** The suggestion currently showing in `state`, or null — for hosts and tests. */
export function currentGhost(state: EditorState): string | null {
  return state.field(ghostField, false)?.text ?? null;
}

/** Accept the ghost text: insert it at the caret as a real edit. */
export const acceptGhostText: Command = (view: EditorView) => {
  const ghost = view.state.field(ghostField, false);
  // The dropdown owns Tab while it is open — defer to its accept.
  if (!ghost || completionStatus(view.state) === "active") return false;
  view.dispatch({
    changes: { from: ghost.pos, insert: ghost.text },
    selection: { anchor: ghost.pos + ghost.text.length },
    userEvent: "input.complete",
  });
  return true;
};

/** Dismiss the ghost text without inserting it. */
export const dismissGhostText: Command = (view: EditorView) => {
  if (!view.state.field(ghostField, false)) return false;
  view.dispatch({ effects: clearGhost.of(null) });
  return true;
};

// Watches typing, paces the source, and shows the survivor. The fetch result is
// discarded when the buffer or caret moved while it was in flight.
function ghostTextTrigger(source: InlineCompletionSource): Extension {
  return ViewPlugin.fromClass(
    class {
      private timer: ReturnType<typeof setTimeout> | null = null;
      private inflight: AbortController | null = null;

      constructor(private readonly view: EditorView) {}

      update(update: ViewUpdate): void {
        if (!update.docChanged) return;
        this.cancel();
        // Typing and deleting re-arm the trigger; accepting a completion (ghost
        // or dropdown) does not — the author asks again by typing on.
        if (
          !update.transactions.some(
            (tr) => tr.isUserEvent("input.type") || tr.isUserEvent("delete"),
          )
        )
          return;
        this.timer = setTimeout(() => void this.request(), source.debounceMs ?? 400);
      }

      private cancel(): void {
        this.inflight?.abort();
        this.inflight = null;
        if (this.timer !== null) clearTimeout(this.timer);
        this.timer = null;
      }

      private async request(): Promise<void> {
        const { state } = this.view;
        if (!state.selection.main.empty) return;
        const pos = state.selection.main.head;
        const doc = state.doc.toString();
        const ctl = new AbortController();
        this.inflight = ctl;
        let text: string;
        try {
          text = (await source.fetch(doc, pos, ctl.signal)).replace(/\n+$/, "");
        } catch (err) {
          // Never blocks typing; a real failure (not our own abort) is the
          // host's to surface.
          if (!ctl.signal.aborted) source.onError?.(err);
          return;
        }
        if (ctl.signal.aborted) return;
        source.onAnswered?.();
        // Stale answer: the buffer or caret moved while the request flew.
        const now = this.view.state;
        if (now.selection.main.head !== pos || now.doc.toString() !== doc) return;
        if (!text.trim()) {
          source.onDrop?.("empty");
          return;
        }
        if (source.validate && !source.validate(doc, pos, text)) {
          source.onDrop?.("invalid");
          return;
        }
        this.view.dispatch({ effects: setGhost.of({ pos, text }) });
        source.onShown?.();
      }

      destroy(): void {
        this.cancel();
      }
    },
  );
}

const ghostTheme = EditorView.theme({
  ".cm-ghost-text": {
    color: "var(--ink-faint)",
    opacity: "0.65",
    fontStyle: "italic",
    whiteSpace: "pre-wrap",
  },
});

/**
 * The inline ghost-text extension. High-precedence keymap so Tab reaches the
 * accept before the editor's indent binding — the dropdown completer still wins
 * while its popup is open, and with no ghost showing both keys fall through.
 */
export function inlineCompletion(source: InlineCompletionSource): Extension {
  return [
    ghostField,
    ghostTextTrigger(source),
    ghostTheme,
    Prec.high(
      keymap.of([
        { key: "Tab", run: acceptGhostText },
        { key: "Escape", run: dismissGhostText },
      ]),
    ),
  ];
}
