import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The problems dock: an introduced error raises the activity-bar badge, lists in
// the dock, and copies to the clipboard. Selectors are data-testid; the error is
// produced by typing an invalid declaration into the open module.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("surfaces an introduced error in the badge and dock", async ({ page }) => {
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\npublic container"); // missing name → parse error

  // The activity-bar badge reflects the new problem count (wasm recompiles, debounced).
  await expect(page.getByTestId("problems-badge")).toBeVisible({ timeout: 15_000 });

  await page.getByTestId("view-problems").click();
  await expect(page.getByTestId("bottom-dock")).toBeVisible();
  await expect(page.getByTestId("problem-0")).toBeVisible();

  // Copy-all writes the formatted problem list to the clipboard.
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.getByTestId("problems-copy-all").click();
  await expect(page.getByText(/Copied \d+ problem/)).toBeVisible();
});
