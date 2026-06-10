import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The doc settings gear hovers over the editor's top-right for Markdown docs only:
// it opens reading-width + the Markdown syntax cheat-sheet in one popover, and it
// never leaks into .pds files or the canvas / 3D views. Selectors are data-testid only.

test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("gear shows on a Markdown doc, sets reading width, and hides on a module", async ({ page }) => {
  // acme-tickets lands on docs/overview.md — a Markdown doc, so the gear is up.
  await createProject(page, "acme-tickets", undefined, { highlight: false });
  const gear = page.getByTestId("doc-settings");
  await expect(gear).toBeVisible();

  // Open the menu and pick Full — the code layer tracks the width choice.
  await gear.click();
  await expect(page.getByTestId("doc-settings-pop")).toBeVisible();
  await page.getByTestId("doc-width-full").click();
  await expect(page.locator('[data-doc-width="full"]')).toHaveCount(1);

  // A .pds module is not a doc — the gear unmounts.
  await page.getByTestId("file-orders").click();
  await expect(gear).toHaveCount(0);
});

test("gear does not leak into the canvas or 3D views", async ({ page }) => {
  await createProject(page, "acme-tickets", undefined, { highlight: false });
  await expect(page.getByTestId("doc-settings")).toBeVisible();

  // The doc stays open in its tab, but the gear belongs to the code view.
  await page.getByTestId("view-canvas").click();
  await expect(page.getByTestId("canvas-view")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("doc-settings")).toBeHidden();

  await page.getByTestId("view-space").click();
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("doc-settings")).toBeHidden();
});
