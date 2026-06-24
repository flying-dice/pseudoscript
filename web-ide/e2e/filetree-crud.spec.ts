import type { Page } from "@playwright/test";
import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The docs/ folder may already be expanded (its landing page auto-opens), so
// toggle it open only when collapsed — an unconditional click would hide it.
async function expandDocs(page: Page): Promise<void> {
  const folder = page.getByTestId("folder-docs");
  if ((await folder.getAttribute("aria-expanded")) !== "true") await folder.click();
}

// File-tree CRUD via the context menu and the shared dialogs: create/delete a
// module and rename/delete an authored doc (issue #69 — docs are first-class
// tree entries, not read-only). All write to the real OPFS workspace and surface
// a success toast.
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
  // Match the row's file field exactly — a substring match over the whole row
  // would hit unrelated message text (e.g. "…by extracting a shared contract").
  await expect(page.getByTestId("problem-0")).toBeVisible({ timeout: 15_000 });
  const fileTags = page.locator('[data-testid^="problem-"][data-testid$="-file"]');
  await expect(fileTags.filter({ hasText: /^extra$/ })).toHaveCount(0);
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

test("creates an authored doc from the command palette", async ({ page }) => {
  await page.keyboard.press("Control+k");
  await page.getByTestId("command-input").fill("New doc");
  await page.getByTestId("cmd-action-new-doc").click();

  const dialog = page.getByTestId("prompt-dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("textbox").fill("Release Notes");
  await dialog.getByRole("textbox").press("Enter");

  await expect(page.getByText("Created docs/release-notes.md")).toBeVisible();
  // The new page auto-opens, expanding docs/ so its row is addressable.
  await expect(page.getByTestId("file-docs/release-notes.md")).toBeVisible();
});

test("renames an authored doc from the context menu", async ({ page }) => {
  await expandDocs(page);
  await page.getByTestId("file-docs/overview.md").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Rename…" }).click();

  const dialog = page.getByTestId("prompt-dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("textbox").fill("Welcome");
  await dialog.getByRole("textbox").press("Enter");

  await expect(page.getByText("Renamed to Welcome")).toBeVisible();
  // The path (and the row that keys off it) is unchanged; only the sidebar label moves.
  await expect(page.getByTestId("file-docs/overview.md")).toBeVisible();
});

test("deletes an authored doc after confirmation", async ({ page }) => {
  await expandDocs(page);
  await page.getByTestId("file-docs/edge-cases.md").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete" }).click();

  const confirm = page.getByTestId("confirm-dialog");
  await expect(confirm).toBeVisible();
  await confirm.getByRole("button", { name: "Delete" }).click();

  await expect(page.getByTestId("file-docs/edge-cases.md")).toBeHidden();
  await expect(page.getByText("Deleted docs/edge-cases.md")).toBeVisible();
});
