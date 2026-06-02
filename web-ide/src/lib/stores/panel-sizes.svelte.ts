// Panel sizes — a reactive rune store, persisted to localStorage.
//
// Owns the draggable widths of the explorer (left) and structure (right) islands
// that flank the centre, and the height of the problems dock at the bottom. Each
// is clamped to a readable range and saved across sessions; the splitter handles
// in the .body gutters write here as you drag.

const EXPLORER_KEY = "pds-explorer-w";
const STRUCTURE_KEY = "pds-structure-w";
const PROBLEMS_KEY = "pds-problems-h";

// Per-panel width bounds (px). The centre is `1fr` and absorbs the slack, so
// these only keep each side legible without crowding the editor.
export const PANEL_MIN = 180;
export const PANEL_MAX = 520;

// Bottom problems-dock height bounds (px). The centre row is `1fr`.
export const PROBLEMS_MIN = 80;
export const PROBLEMS_MAX = 600;

const EXPLORER_DEFAULT = 248;
const STRUCTURE_DEFAULT = 268;
const PROBLEMS_DEFAULT = 200;

const clamp = (n: number) => Math.min(PANEL_MAX, Math.max(PANEL_MIN, Math.round(n)));
const clampH = (n: number) => Math.min(PROBLEMS_MAX, Math.max(PROBLEMS_MIN, Math.round(n)));

function load(key: string, fallback: number, fit: (n: number) => number = clamp): number {
  try {
    const v = Number(localStorage.getItem(key));
    return Number.isFinite(v) && v > 0 ? fit(v) : fallback;
  } catch {
    return fallback;
  }
}

function save(key: string, value: number): void {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    /* private mode / quota — applies this session only */
  }
}

class PanelSizes {
  // The explorer (left) island width (persisted).
  explorerW = $state<number>(load(EXPLORER_KEY, EXPLORER_DEFAULT));
  // The structure (right) island width (persisted).
  structureW = $state<number>(load(STRUCTURE_KEY, STRUCTURE_DEFAULT));
  // The problems-dock (bottom) island height (persisted).
  problemsH = $state<number>(load(PROBLEMS_KEY, PROBLEMS_DEFAULT, clampH));

  setExplorerW(px: number): void {
    this.explorerW = clamp(px);
    save(EXPLORER_KEY, this.explorerW);
  }

  setStructureW(px: number): void {
    this.structureW = clamp(px);
    save(STRUCTURE_KEY, this.structureW);
  }

  resetExplorer(): void {
    this.setExplorerW(EXPLORER_DEFAULT);
  }

  resetStructure(): void {
    this.setStructureW(STRUCTURE_DEFAULT);
  }

  setProblemsH(px: number): void {
    this.problemsH = clampH(px);
    save(PROBLEMS_KEY, this.problemsH);
  }

  resetProblems(): void {
    this.setProblemsH(PROBLEMS_DEFAULT);
  }
}

export const panelSizes = new PanelSizes();
