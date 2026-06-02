// Canvas scene projection — pure.
//
// Projects the selected node (or the whole-model context) to a laid-out scene:
// the compiler picks the view, a sequence scene is collapsed to the chosen depth,
// and a symbol with nothing to draw falls back to its own lifeline. The WASM scene
// functions are injected; errors are reported via an injected callback (wired to
// the error registry by the caller) so this stays side-effect-free. No Svelte.

import { collapseSequence } from "../sequence.js";
import { singleLifelineScene, type ModelIndex } from "./model.js";
import type { Canvas, Depth, Module, Scene } from "./types.js";

// The WASM scene functions the projection needs.
export type CanvasWasm = {
  symbolScene: (modules: Module[], fqn: string) => Scene;
  emitSceneModules: (modules: Module[], view: string, target?: string) => Scene;
  layoutScene: (scene: Scene) => Scene;
};

export type ProjectCanvasArgs = {
  selected: { fqn: string } | null;
  seqDepth: Depth;
  modules: Module[];
  index: ModelIndex;
  wasm: CanvasWasm;
  onError: (code: "DIAGRAM_PROJECTION_FAILED" | "DIAGRAM_RENDER_FAILED", detail: string) => void;
};

/**
 * Project the canvas scene + layout. A selected symbol projects its fitting view;
 * no selection projects the context overview. A sequence is collapsed to `seqDepth`
 * and positioned by the layout engine; C4 stays as-is. An empty or unprojectable
 * selected symbol falls back to its single lifeline; an unprojectable context is an
 * error. Both error paths report via `onError`.
 */
export function projectCanvas(args: ProjectCanvasArgs): Canvas {
  const { selected, seqDepth, modules, index, wasm, onError } = args;
  const lifelineFallback = (): Canvas => {
    const scene = singleLifelineScene(index, selected!.fqn);
    return { scene, layout: wasm.layoutScene(scene), error: "" };
  };
  try {
    const raw = selected ? wasm.symbolScene(modules, selected.fqn) : wasm.emitSceneModules(modules, "context", "");
    const isSeq = !!raw && Array.isArray(raw.participants);
    const shown = (isSeq ? collapseSequence(raw as never, seqDepth, index.nodeInfo) : raw) as Scene | null;
    const isEmpty = isSeq
      ? !(shown?.participants as unknown[] | undefined)?.length
      : !(shown?.nodes as unknown[] | undefined)?.length;
    if (isEmpty && selected) return lifelineFallback();
    const layout = isSeq && shown ? wasm.layoutScene(shown) : null;
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
