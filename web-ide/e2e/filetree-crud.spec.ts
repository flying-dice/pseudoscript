import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// File-tree CRUD via the context menu and the shared dialogs: create a module
// (PromptDialog) and delete one (ConfirmDialog). Both write to the real OPFS
// workspace and surface a success toast.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("creates a new module from the context menu", async ({ page }) => {
  await page.getByTestId("file-orders").click({ button: "right" });
  await page.getByRole("menuitem", { name: "New file…" }).click();

  const dialog = page.getByTestId("prompt-dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("textbox").fill("extra");
  await dialog.getByRole("textbox").press("Enter");

  await expect(page.getByTestId("file-extra")).toBeVisible();
  await expect(page.getByText("Created extra.pds")).toBeVisible();

  // The scaffolded module checks clean — no problem row names it (issue #49:
  // the skeleton must fully qualify its `for` parent and type every callable).
  // The sample's own architectural warnings may populate the dock; only rows
  // for the new module would betray an invalid scaffold.
  await page.getByTestId("view-problems").click();
  await expect(page.getByTestId("bottom-dock")).toBeVisible();
  // Anchor on the sample's own architectural warnings so diagnostics have
  // demonstrably arrived before asserting the new module contributes none.
  await expect(page.getByTestId("problem-0")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('[data-testid^="problem-"]').filter({ hasText: "extra" })).toHaveCount(0);
});

test("deletes a module after confirmation", async ({ page }) => {
  await page.getByTestId("file-payments").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete" }).click();

  const confirm = page.getByTestId("confirm-dialog");
  await expect(confirm).toBeVisible();
  await confirm.getByRole("button", { name: "Delete" }).click();

  await expect(page.getByTestId("file-payments")).toBeHidden();
  await expect(page.getByText("Deleted payments")).toBeVisible();
});
