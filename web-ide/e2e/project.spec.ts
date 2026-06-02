import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The launcher is disk-first: open a folder, re-open a recent, or start a New
// project from a template. These drive the real flow against an OPFS tmp folder
// (see harness.ts) — the picker's destination is the only stub.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("the launcher offers Open / New, and New opens the template dialog", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("open-folder")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("new-project")).toBeVisible();
  // Templates are no longer on the launcher — they live in the New-project dialog.
  await expect(page.getByTestId("template-empty")).toHaveCount(0);

  await page.getByTestId("new-project").click();
  await expect(page.getByTestId("template-empty")).toBeVisible();
  await expect(page.getByTestId("template-acme-tickets")).toBeVisible();
  // Name and target folder are both mandatory: templates stay disabled until both
  // are set.
  await expect(page.getByTestId("template-empty")).toBeDisabled();
  await page.getByTestId("new-project-name").fill("demo");
  await expect(page.getByTestId("template-empty")).toBeDisabled();
  await page.getByTestId("choose-folder").click();
  await expect(page.getByTestId("template-empty")).toBeEnabled();
});

test("the empty template bootstraps a one-module project on disk", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  // the one-file starter — a valid PseudoScript model
  await expect(content).toContainText("public container Api");
  await expect(page.getByTestId("file-main")).toBeVisible();
});

test("edits persist only on save — no autosave; Cmd/Ctrl-S writes to disk", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  await expect(content).toContainText("public container Api");

  const readDisk = () =>
    page.evaluate(async () => {
      const proj = await window.__lastPicked!.getDirectoryHandle("my-architecture");
      const file = await proj.getFileHandle("main.pds");
      return (await file.getFile()).text();
    });

  // Edit in the editor: append a comment line (keeps the model valid).
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\n// SAVE_MARKER");

  // No autosave: the active tab reads dirty and disk is untouched even after the
  // old debounce window would have elapsed.
  await expect(page.locator(".tab.active .tab-dirty")).toBeVisible();
  await page.waitForTimeout(800);
  expect(await readDisk()).not.toContain("SAVE_MARKER");

  // Explicit save writes the buffer and clears the dirty marker.
  await page.keyboard.press("ControlOrMeta+s");
  await expect(page.locator(".tab.active .tab-dirty")).toHaveCount(0);
  expect(await readDisk()).toContain("SAVE_MARKER");
});

test("a file edited outside the IDE is reloaded", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  await expect(content).toContainText("public container Api");

  // Edit main.pds on disk, outside the editor — through the very folder the IDE
  // scaffolded into (the stubbed OPFS pick).
  await page.evaluate(async () => {
    const proj = await window.__lastPicked!.getDirectoryHandle("my-architecture");
    const file = await proj.getFileHandle("main.pds");
    const writable = await file.createWritable();
    await writable.write("//! main\n\npublic system ReloadedFromDisk;\n");
    await writable.close();
  });

  // Regaining focus pulls in external changes immediately.
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await expect(content).toContainText("ReloadedFromDisk");
  await expect(content).not.toContainText("public container Api");
});

test("File ▸ Close project tears down the workspace and reopens the launcher", async ({ page }) => {
  await createProject(page, "empty");
  await expect(page.getByTestId("editor").locator(".cm-content")).toContainText("public container Api");

  await page.getByRole("button", { name: "File", exact: true }).click();
  await page.getByRole("menuitem", { name: "Close project" }).click();

  // A clean workspace closes without a confirm; the launcher returns.
  await expect(page.getByTestId("open-folder")).toBeVisible();
  await expect(page.getByTestId("new-project")).toBeVisible();
});

test("Close project with unsaved edits asks before discarding", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  await expect(content).toContainText("public container Api");

  // Dirty the active buffer.
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\n// CLOSE_MARKER");
  await expect(page.locator(".tab.active .tab-dirty")).toBeVisible();

  await page.getByRole("button", { name: "File", exact: true }).click();
  await page.getByRole("menuitem", { name: "Close project" }).click();

  // The destructive-confirm appears; confirming closes to the launcher.
  const confirm = page.getByTestId("confirm-dialog");
  await expect(confirm.getByText("Discard unsaved changes?")).toBeVisible();
  await confirm.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(page.getByTestId("open-folder")).toBeVisible();
});
