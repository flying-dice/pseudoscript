// Canvas scene projection — pure.
//
// Projects the selected node (or the whole-model context) to a laid-out scene:
// the compiler picks the view, a sequence scene is collapsed to the chosen depth,
// and a symbol with nothing to draw falls back to its own lifeline. The WASM scene
// functions are injected; errors are reported via an injected callback (wired to
// the error registry by the caller) so this stays side-effect-free. No Svelte.

import { collapseSequence } from "../sequence.js";
import { singleLifelineScene, type ModelIndex } from "./model.js";
import type { Canvas, Depth, LayoutTweaks, Scene } from "./types.js";

// The scene functions the projection needs. The session holds the workspace, so
// these take only the view/symbol — no modules argument.
export type CanvasWasm = {
  symbolScene: (fqn: string) => Scene;
  emitScene: (view: string, target?: string) => Scene;
  layoutScene: (scene: Scene, tweaks?: LayoutTweaks) => Scene;
};

export type ProjectCanvasArgs = {
  selected: { fqn: string } | null;
  seqDepth: Depth;
  index: ModelIndex;
  wasm: CanvasWasm;
  // C4 layout tweaks for the current diagram (the "Layout" control). Optional;
  // omitted = engine defaults.
  tweaks?: LayoutTweaks;
  onError: (code: "DIAGRAM_PROJECTION_FAILED" | "DIAGRAM_RENDER_FAILED", detail: string) => void;
};

/**
 * Project the canvas scene + layout. A selected symbol projects its fitting view;
 * no selection projects the context overview. A sequence is collapsed to `seqDepth`;
 * both kinds are then positioned by the Rust layout engine. An empty or
 * unprojectable selected symbol falls back to its single lifeline; an
 * unprojectable context is an error. Both error paths report via `onError`.
 */
export function projectCanvas(args: ProjectCanvasArgs): Canvas {
  const { selected, seqDepth, index, wasm, tweaks, onError } = args;
  // The fallback lays out a synthesised scene, which can itself throw (an
  // unlayoutable scene). Guard it: a throw here must not escape — invoked from
  // the catch below it would otherwise be uncaught and crash the canvas.
  const lifelineFallback = (): Canvas => {
    try {
      const scene = singleLifelineScene(index, selected!.fqn);
      return { scene, layout: wasm.layoutScene(scene, tweaks), error: "" };
    } catch (e) {
      const detail = String((e as Error)?.message ?? e);
      onError("DIAGRAM_RENDER_FAILED", detail);
      return { scene: null, layout: null, error: detail };
    }
  };
  try {
    const raw = selected ? wasm.symbolScene(selected.fqn) : wasm.emitScene("context", "");
    const isSeq = !!raw && Array.isArray(raw.participants);
    const shown = (isSeq ? collapseSequence(raw as never, seqDepth, index.nodeInfo) : raw) as Scene | null;
    // A scene is empty when its discriminating collection is empty: participants
    // for a sequence, entities for a data ER view, steps for a feature flow, else
    // nodes for a C4 view. An empty selected symbol falls back to its lifeline.
    const len = (key: string): number => (shown?.[key] as unknown[] | undefined)?.length ?? 0;
    const isEmpty = isSeq
      ? len("participants") === 0
      : Array.isArray(shown?.entities)
        ? len("entities") === 0
        : Array.isArray(shown?.steps)
          ? len("steps") === 0
          : len("nodes") === 0;
    if (isEmpty && selected) return lifelineFallback();
    // Both kinds are positioned by the Rust layout engine: a sequence yields a
    // positioned timeline, a C4 scene yields placed cards + routed edges (the
    // same geometry the static SVG draws).
    const layout = shown ? wasm.layoutScene(shown, tweaks) : null;
    return { scene: shown, layout, error: "" };
  } catch (e) {
    const detail = String((e as Error)?.message ?? e);
    if (selected) {
      onError("DIAGRAM_PROJECTION_FAILED", `${selected.fqn}: ${detail}`);
      return lifelineFallback();
    }
    onError("DIAGRAM_RENDER_FAILED", detail);
    return { scene: null, layout: null, error: detail };
  }
}

/** The placeholder hint shown when the canvas is empty. */
export function canvasHint(selected: unknown): string {
  return selected
    ? "Nothing to draw for this item."
    : "No persons or systems declared yet — the context overview draws systems and people.";
}
