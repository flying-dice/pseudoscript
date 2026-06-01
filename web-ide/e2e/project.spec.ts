import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The launcher is disk-first: open a folder, re-open a recent, or start a New
// project from a template. These drive the real flow against an OPFS tmp folder
// (see harness.ts) — the picker's destination is the only stub.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("the launcher starts a new project from a template", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("template-empty")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("template-acme-tickets")).toBeVisible();
  await expect(page.getByTestId("open-folder")).toBeVisible();
});

test("the empty template bootstraps a one-module project on disk", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  // the one-file starter — a valid PseudoScript model
  await expect(content).toContainText("public container Api");
  await expect(page.getByTestId("file-main")).toBeVisible();
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
