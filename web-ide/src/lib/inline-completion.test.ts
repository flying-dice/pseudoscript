import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EditorView } from "@codemirror/view";

import {
  acceptGhostText,
  currentGhost,
  dismissGhostText,
  inlineCompletion,
} from "./inline-completion.js";
import type { InlineCompletionSource } from "./inline-completion.js";

// State-level coverage: the trigger/abort/stale logic, the ghost field, and the
// accept/dismiss commands. Widget geometry needs a live browser — that side is
// e2e-covered (ai-completion.spec.ts).

let view: EditorView;
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  view?.destroy();
  vi.useRealTimers();
});

function editor(source: Partial<InlineCompletionSource>, doc = ""): EditorView {
  view = new EditorView({
    doc,
    extensions: inlineCompletion({ fetch: () => Promise.resolve(""), ...source }),
    parent: document.body,
  });
  return view;
}

function type(v: EditorView, text: string): void {
  const at = v.state.selection.main.head;
  v.dispatch({
    changes: { from: at, insert: text },
    selection: { anchor: at + text.length },
    userEvent: "input.type",
  });
}

// Let the debounce fire and the fetch promise chain settle.
async function settle(ms = 400): Promise<void> {
  await vi.advanceTimersByTimeAsync(ms);
  await vi.advanceTimersByTimeAsync(0);
}

describe("trigger", () => {
  it("fetches after the idle debounce and shows the suggestion", async () => {
    const fetch = vi.fn().mockResolvedValue("system Banking;");
    editor({ fetch });
    type(view, "public ");
    expect(fetch).not.toHaveBeenCalled(); // not before the idle pause
    await settle();
    expect(fetch).toHaveBeenCalledOnce();
    expect(fetch.mock.calls[0][0]).toBe("public ");
    expect(fetch.mock.calls[0][1]).toBe(7);
    expect(currentGhost(view.state)).toBe("system Banking;");
  });

  it("debounces: only the last keystroke fetches", async () => {
    const fetch = vi.fn().mockResolvedValue("x");
    editor({ fetch });
    type(view, "a");
    await vi.advanceTimersByTimeAsync(200);
    type(view, "b");
    await settle();
    expect(fetch).toHaveBeenCalledOnce();
    expect(fetch.mock.calls[0][0]).toBe("ab");
  });

  it("aborts the in-flight request on the next keystroke", async () => {
    let signal: AbortSignal | undefined;
    const fetch = vi.fn().mockImplementation((_d: string, _p: number, s: AbortSignal) => {
      signal = s;
      return new Promise<string>(() => {}); // never resolves
    });
    editor({ fetch });
    type(view, "a");
    await settle();
    expect(signal!.aborted).toBe(false);
    type(view, "b");
    expect(signal!.aborted).toBe(true);
  });

  it("ignores a programmatic (non-user) change", async () => {
    const fetch = vi.fn().mockResolvedValue("x");
    editor({ fetch });
    view.dispatch({ changes: { from: 0, insert: "loaded" } });
    await settle();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("discards a stale answer when the caret moved while it flew", async () => {
    let resolve!: (s: string) => void;
    const fetch = vi.fn().mockReturnValue(new Promise<string>((r) => (resolve = r)));
    editor({ fetch });
    type(view, "a");
    await settle();
    view.dispatch({ selection: { anchor: 0 } }); // caret moves, no edit
    resolve("late");
    await settle(0);
    expect(currentGhost(view.state)).toBeNull();
  });

  it("swallows a rejecting source", async () => {
    editor({ fetch: vi.fn().mockRejectedValue(new Error("offline")) });
    type(view, "a");
    await settle();
    expect(currentGhost(view.state)).toBeNull();
  });

  it("drops blank and validator-rejected suggestions", async () => {
    const validate = vi.fn().mockReturnValue(false);
    editor({ fetch: vi.fn().mockResolvedValue("   \n"), validate });
    type(view, "a");
    await settle();
    expect(currentGhost(view.state)).toBeNull();
    expect(validate).not.toHaveBeenCalled(); // blank dies before the validator

    editor({ fetch: vi.fn().mockResolvedValue("bad()"), validate });
    type(view, "a");
    await settle();
    expect(validate).toHaveBeenCalledWith("a", 1, "bad()");
    expect(currentGhost(view.state)).toBeNull();
  });
});

describe("accept / dismiss", () => {
  async function withGhost(text: string): Promise<EditorView> {
    editor({ fetch: vi.fn().mockResolvedValue(text) }, "");
    type(view, "x");
    await settle();
    expect(currentGhost(view.state)).toBe(text);
    return view;
  }

  it("Tab-accept inserts the text at the caret and clears the ghost", async () => {
    const v = await withGhost(" = 1");
    expect(acceptGhostText(v)).toBe(true);
    expect(v.state.doc.toString()).toBe("x = 1");
    expect(v.state.selection.main.head).toBe(5);
    expect(currentGhost(v.state)).toBeNull();
  });

  it("Escape dismisses without inserting", async () => {
    const v = await withGhost(" = 1");
    expect(dismissGhostText(v)).toBe(true);
    expect(v.state.doc.toString()).toBe("x");
    expect(currentGhost(v.state)).toBeNull();
  });

  it("both fall through when no ghost is showing", () => {
    editor({});
    expect(acceptGhostText(view)).toBe(false);
    expect(dismissGhostText(view)).toBe(false);
  });

  it("any further edit clears the ghost", async () => {
    const v = await withGhost(" = 1");
    type(v, "y");
    expect(currentGhost(v.state)).toBeNull();
  });

  it("accepting does not immediately refetch", async () => {
    const fetch = vi.fn().mockResolvedValue(" = 1");
    editor({ fetch });
    type(view, "x");
    await settle();
    expect(currentGhost(view.state)).toBe(" = 1");
    fetch.mockClear();
    acceptGhostText(view);
    await settle();
    expect(fetch).not.toHaveBeenCalled();
    expect(currentGhost(view.state)).toBeNull();
  });
});
