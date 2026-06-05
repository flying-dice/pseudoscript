// Shared types for the framework-agnostic IDE core.
//
// Pure type declarations — no runtime, no Svelte. The view layer (`+page.svelte`,
// stores) and the pure `core/` modules both import from here, so a concern can be
// lifted out of the view without dragging Svelte or the WASM module with it.

import type { Module, Occurrence, Scene } from "$lib/pds.js";
import type { Depth, Info } from "$lib/sequence.js";

// Re-export the lib shapes the core builds on, so core modules import everything
// from one place (`$lib/core/types`) rather than reaching into each lib.
export type { Module, Occurrence, Scene, Depth, Info };

// A structural node listed by `outline`/`outlineModules`. The wasm payload carries
// `line`, `col`, and `parent` beyond `pds.ts`'s narrower `OutlineNode`.
export type StructureNode = {
  fqn: string;
  name: string;
  kind: string;
  triggered: boolean;
  line: number;
  col: number;
  parent?: string | null;
  summary?: string | null;
};

// A structure node tagged with its declaring file's FQN.
export type Symbol = StructureNode & { fileFqn: string };

// A workspace diagnostic from `checkModules` (1-based positions), tagged with its
// owning module's FQN.
export type Problem = {
  severity: string;
  message: string;
  start_line: number;
  start_col: number;
  end_line?: number;
  end_col?: number;
  code?: string;
  file?: string;
};

// An open-file descriptor — a module, an authored doc page, or the manifest. The
// `isDoc`/`isManifest` discriminants gate which fields apply.
export type OpenFile = {
  path?: string;
  fqn?: string;
  handle?: FileSystemFileHandle | null;
  title?: string;
  isDoc?: boolean;
  isManifest?: boolean;
  // A non-PDS companion file (editable text). `binary` ones show an inert
  // placeholder (with `bytes`, the on-disk size) instead of the editor.
  isOther?: boolean;
  binary?: boolean;
  bytes?: number;
};

// The live workspace: a real on-disk workspace or an in-memory sample/share. The
// superset of fields the view reads; on-disk-only fields optional.
export type WorkspaceModel = {
  name: string;
  files: OpenFile[];
  // Base-relative directories on disk (empty ones included). Absent for in-memory
  // sample / share-link workspaces, where folders are implied by file paths.
  dirs?: string[];
  manifestToml?: string | null;
  root?: FileSystemDirectoryHandle | null;
  base?: string;
  manifest?: { handle?: FileSystemFileHandle | null; path: string } | null;
  docs?: Record<string, string>;
  // Non-PDS companion files surfaced in the tree (editable text / inert binary).
  // Absent for in-memory sample / share-link workspaces.
  others?: OpenFile[];
};

// A live doc sidebar item / group (handles optional for sample pages).
export type LiveDocItem = { title: string; path: string; handle?: FileSystemFileHandle | null };
export type LiveDocGroup = { title: string; items: LiveDocItem[] };

// A location recorded in / replayed from navigation history. `view` is which
// pane it was visited in — `"canvas"` entries replay the diagram scope (`fqn`,
// or the whole-model overview when absent); absent ⇒ `"code"`.
export type Loc = {
  fileFqn: string;
  line: number;
  col: number;
  label?: string;
  fqn?: string;
  view?: "code" | "canvas";
};

// The editor's imperative API, handed back via `onready`. Impure (DOM/CodeMirror) —
// held by the view/stores, injected into core only as plain values it returns.
export type EditorApi = {
  goto: (line: number, col: number) => void;
  location: () => { line: number; col: number } | null;
  openSettings: () => void;
};

// A laid-out diagram scene + its positioned layout, or an error to show instead.
export type Canvas = { scene: Scene | null; layout?: Scene | null; error: string };

// Per-diagram C4 layout tweaks (the canvas "Layout" control). Applies to C4
// views only; the layout engine ignores it for other scene kinds.
export type LayoutTweaks = {
  minimizeLongEdges: boolean;
  orientation: "tb" | "lr";
  spacing: "compact" | "comfortable" | "roomy";
  /** Brute-force grid placement instead of dot. */
  experimentalGrid: boolean;
  /** Grid cost dials (apply when {@link experimentalGrid}): the weight of a
   *  crossing, a cell of edge length, and a cell of against-the-flow travel. */
  gridCrossingCost: number;
  gridDistanceCost: number;
  gridFlowCost: number;
  /** Grid search: `"auto"`, `"heuristic"`, or `"exhaustive"` (brute force where
   *  feasible) — the toggle for checking the heuristic against exact. */
  gridSearch: "auto" | "heuristic" | "exhaustive";
  /** Grid pins for the current view (drag-to-pin). Not persisted in the global
   *  tweaks blob — supplied per-view from the pin store at layout time. */
  gridPins?: import("$lib/core/pins.js").Pin[];
};

export const DEFAULT_LAYOUT_TWEAKS: LayoutTweaks = {
  minimizeLongEdges: false,
  orientation: "tb",
  spacing: "comfortable",
  experimentalGrid: false,
  gridCrossingCost: 10,
  gridDistanceCost: 1,
  gridFlowCost: 5,
  gridSearch: "auto",
};

// The canvas find-usages popover: a reference list anchored at the pointer.
export type CanvasUsages = { name: string; items: Occurrence[]; x: number; y: number };

// The right-click menu, shared by the C4 graph and the sequence timeline. A
// target is the identity of the symbol clicked; a request opens the menu for it
// at the event point. A row carries its action plus optional adornments (a
// leading glyph, a trailing badge); a section is a rule-separated group of rows.
export type MenuTarget = { fqn: string; kind: string; label: string };
export type MenuRequest = (target: MenuTarget, event: MouseEvent) => void;
export type MenuItem = { label: string; run: () => void; icon?: string; badge?: string };
export type MenuSection = { label?: string; items: MenuItem[] };

// A FileTree name-prompt dialog config, and the destructive-confirm config.
export type Dialog = {
  title: string;
  label: string;
  placeholder: string;
  value: string;
  confirmLabel: string;
  hint: string;
  validate: (name: string) => string | null;
  run: (name: string) => void;
};
export type ConfirmDialog = { title: string; message: string; confirmLabel?: string; run: () => void };

// A toast notification.
export type NoteKind = "success" | "error" | "info";
export type Note = { id: number; kind: NoteKind; title: string; body: string };

// A pending debounced disk write.
export type PendingWrite = { handle: FileSystemFileHandle; key: string; text: string };

// The injected subset of `pds.ts` the core depends on. Passing this as a value
// (rather than importing `$lib/pds.js`) keeps core unit-testable with fakes and
// isolates the WASM lifecycle to one store. `outlineModules` is typed to the wider
// `StructureNode` the wasm payload actually carries (the `wasm` store adapts it).
export interface WasmApi {
  checkModules: (modules: Module[]) => { fqn: string; diagnostics: Problem[] }[];
  outlineModules: (modules: Module[]) => StructureNode[];
  symbolScene: (modules: Module[], fqn: string) => Scene;
  emitSceneModules: (modules: Module[], view: string, target?: string) => Scene;
  layoutScene: (scene: Scene) => Scene;
  hover: (modules: Module[], moduleFqn: string, offset: number) => unknown;
  references: (modules: Module[], moduleFqn: string, offset: number) => unknown;
  docManifest: (toml: string) => unknown;
}
