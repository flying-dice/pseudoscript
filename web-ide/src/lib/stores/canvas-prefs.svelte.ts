// Canvas rendering preferences — a reactive rune store, persisted to localStorage.
//
// Owns the C4 diagram's layout algorithm, its flow direction, and its edge line
// style. All three are saved across sessions and edited from the canvas's
// "Customise" modal. C4Flow reads these and re-projects when any of them change.

// The placement algorithm for the flat (peer) view. "layered" is dagre's
// hierarchical layout (the only one that honours `direction`); the others are
// geometric and ignore direction.
export type LayoutAlgo = "layered" | "grid" | "circular" | "radial";
// The dagre rank direction a layered graph flows along.
export type LayoutDir = "TB" | "LR" | "BT" | "RL";
// The edge routing style (mapped to a Svelte Flow built-in edge type).
export type EdgeStyle = "smoothstep" | "bezier" | "straight" | "step";

// Algorithm options, in menu order. `directional` flags the ones that use the
// direction setting, so the modal can disable the direction control otherwise.
export const ALGORITHMS: readonly { id: LayoutAlgo; label: string; directional: boolean }[] = [
  { id: "layered", label: "Layered", directional: true },
  { id: "grid", label: "Grid", directional: false },
  { id: "circular", label: "Circular", directional: false },
  { id: "radial", label: "Radial", directional: false },
];
// Direction options, in menu order.
export const LAYOUTS: readonly { id: LayoutDir; label: string }[] = [
  { id: "TB", label: "Top → bottom" },
  { id: "LR", label: "Left → right" },
  { id: "BT", label: "Bottom → top" },
  { id: "RL", label: "Right → left" },
];
// Line-style options, in menu order.
export const EDGE_STYLES: readonly { id: EdgeStyle; label: string }[] = [
  { id: "smoothstep", label: "Smooth step" },
  { id: "bezier", label: "Bezier" },
  { id: "straight", label: "Straight" },
  { id: "step", label: "Step" },
];

const ALGO_KEY = "pds-canvas-algo";
const LAYOUT_KEY = "pds-canvas-layout";
const EDGE_KEY = "pds-canvas-edge";

const isAlgo = (v: string | null): v is LayoutAlgo => ALGORITHMS.some((a) => a.id === v);
const isLayout = (v: string | null): v is LayoutDir => LAYOUTS.some((l) => l.id === v);
const isEdge = (v: string | null): v is EdgeStyle => EDGE_STYLES.some((e) => e.id === v);

function load<T extends string>(key: string, guard: (v: string | null) => v is T, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return guard(v) ? v : fallback;
  } catch {
    return fallback;
  }
}

function save(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* private mode / quota — applies this session only */
  }
}

/** Whether an algorithm honours the direction setting. */
export function isDirectional(algo: LayoutAlgo): boolean {
  return ALGORITHMS.find((a) => a.id === algo)?.directional ?? false;
}

class CanvasPrefs {
  // The flat-view placement algorithm (persisted).
  algorithm = $state<LayoutAlgo>(load(ALGO_KEY, isAlgo, "layered"));
  // The layered graph's flow direction (persisted).
  layout = $state<LayoutDir>(load(LAYOUT_KEY, isLayout, "TB"));
  // The edge line style (persisted).
  edge = $state<EdgeStyle>(load(EDGE_KEY, isEdge, "smoothstep"));

  /** The Svelte Flow edge `type` for the chosen style — bezier is the built-in "default". */
  get edgeType(): string {
    return this.edge === "bezier" ? "default" : this.edge;
  }

  setAlgorithm(algo: LayoutAlgo): void {
    this.algorithm = algo;
    save(ALGO_KEY, algo);
  }

  setLayout(dir: LayoutDir): void {
    this.layout = dir;
    save(LAYOUT_KEY, dir);
  }

  setEdge(style: EdgeStyle): void {
    this.edge = style;
    save(EDGE_KEY, style);
  }
}

export const canvasPrefs = new CanvasPrefs();
