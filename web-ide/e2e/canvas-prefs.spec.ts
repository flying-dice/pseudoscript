import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The canvas "Customise" modal owns the layout algorithm, its direction, and the
// edge line style. This pins that the modal drives the canvas-prefs store and
// that every choice persists to localStorage across sessions.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("the Customise modal sets algorithm, direction, and line style — all persisted", async ({ page }) => {
  await createProject(page, "empty");
  await page.getByLabel("Canvas").click();
  await expect(page.locator(".svelte-flow__node-card").first()).toBeVisible({ timeout: 20_000 });

  await page.getByRole("button", { name: "Customise diagram" }).click();
  const dialog = page.getByTestId("canvas-settings");
  await expect(dialog).toBeVisible();

  // A geometric algorithm persists and disables the direction control.
  await dialog.locator("#cs-algo").selectOption("grid");
  await dialog.locator("#cs-edge").selectOption("straight");
  expect(await page.evaluate(() => localStorage.getItem("pds-canvas-algo"))).toBe("grid");
  expect(await page.evaluate(() => localStorage.getItem("pds-canvas-edge"))).toBe("straight");
  await expect(dialog.locator("#cs-dir")).toBeDisabled();

  // The layered algorithm re-enables direction, which persists too.
  await dialog.locator("#cs-algo").selectOption("layered");
  await dialog.locator("#cs-dir").selectOption("LR");
  expect(await page.evaluate(() => localStorage.getItem("pds-canvas-layout"))).toBe("LR");
});
