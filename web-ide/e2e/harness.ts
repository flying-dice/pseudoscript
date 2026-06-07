import { type Page, expect } from "@playwright/test";

// The IDE is disk-only: every project is a real folder reached through the File
// System Access API. The native directory picker can't be driven headless, so we
// stub only its *destination* to a fresh OPFS directory — the browser's real,
// on-disk, per-origin filesystem. Everything downstream (scaffold writes, reads,
// manual save, external-change reload) runs against real FileSystemDirectoryHandles;
// only where the picker points is stubbed, exactly as a native dialog chooses.
//
// The picked handle is stashed on `window.__lastPicked` so a test can reach the
// same files the IDE wrote — e.g. to simulate an edit made outside the IDE.
declare global {
  interface Window {
    __lastPicked?: FileSystemDirectoryHandle;
  }
}

export async function stubPicker(page: Page): Promise<void> {
  await page.addInitScript(() => {
    window.showDirectoryPicker = async () => {
      const root = await navigator.storage.getDirectory();
      // A unique parent each call so `scaffoldWorkspace` creates its <name> dir
      // cleanly and tests stay isolated even within one browser context.
      const name = "e2e-" + Math.random().toString(36).slice(2);
      const dir = await root.getDirectoryHandle(name, { create: true });
      window.__lastPicked = dir;
      return dir;
    };
  });
}

// Boots the IDE and creates a project from `templateId` (the launcher's New-project
// flow), optionally opening a module by fqn. Resolves once a module is visible and
// highlighted — i.e. wasm is ready and the disk-backed workspace is mounted.
//
// `highlight` (default true) waits for a semantic-token highlight as the wasm-ready
// signal. A template whose landing file is a Markdown doc (e.g. acme-tickets'
// `docs/overview.md`) has no `.pds` highlight, so such callers pass `false` and
// gate readiness on their own post-condition (e.g. a projected canvas node).
export async function createProject(
  page: Page,
  templateId: string,
  openFqn?: string,
  { highlight = true }: { highlight?: boolean } = {},
): Promise<void> {
  await page.goto("/");
  // The launcher only offers Open/Recent/New; templates live in the New-project
  // dialog and stay disabled until a name and a target folder are both set. The
  // folder picker resolves to the stubbed OPFS dir; the project scaffolds inside it.
  await page.getByTestId("new-project").click({ timeout: 30_000 });
  await page.getByTestId("new-project-name").fill("my-architecture");
  await page.getByTestId("choose-folder").click();
  await page.getByTestId(`template-${templateId}`).click({ timeout: 30_000 });
  if (openFqn) await page.getByTestId(`file-${openFqn}`).click();
  // Scaffolding the template to OPFS, mounting it, and producing the first wasm
  // highlights can take a while on a loaded CI machine — wait generously.
  const content = page.getByTestId("editor").locator(".cm-content");
  await expect(content).toBeVisible({ timeout: 20_000 });
  if (highlight) {
    await expect(page.locator('[data-sem="keyword"]').first()).toBeVisible({ timeout: 20_000 });
  }
}
