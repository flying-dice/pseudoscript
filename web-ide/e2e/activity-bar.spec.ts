import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The left activity rail switches the centre between the explorer/editor, the
// canvas, and the 3D graph, and toggles the problems dock.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("switches the centre view between canvas and 3D space", async ({ page }) => {
  await page.getByTestId("view-canvas").click();
  await expect(page.getByTestId("canvas-view")).toBeVisible();

  await page.getByTestId("view-space").click();
  await expect(page.getByTestId("space-view")).toBeVisible();
});

test("toggles the problems dock", async ({ page }) => {
  await expect(page.getByTestId("bottom-dock")).toBeHidden();
  await page.getByTestId("view-problems").click();
  await expect(page.getByTestId("bottom-dock")).toBeVisible();
  await page.getByTestId("view-problems").click();
  await expect(page.getByTestId("bottom-dock")).toBeHidden();
});
