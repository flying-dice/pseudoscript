import { expect, test } from "@playwright/test";

// The project launcher opens on a fresh load (no workspace yet). These tests
// drive it purely through data-testid. The actual disk scaffold goes through the
// File System Access picker, which can't be driven headless — so coverage stops
// at the affordance: the "New workspace" action reveals a named create form.
test.beforeEach(async ({ page }) => {
  await page.goto("/");
  // The launcher only renders once wasm has initialised; a cold dev server can
  // take several seconds to compile and stream it.
  await expect(page.getByTestId("new-workspace")).toBeVisible({ timeout: 30000 });
});

test("the launcher offers initialising a new workspace on disk", async ({ page }) => {
  const start = page.getByTestId("new-workspace");
  await expect(start).toBeEnabled();
  await start.click();

  // The action expands into an inline, focused name field plus a create button —
  // no second modal stacked over the launcher.
  const name = page.getByTestId("new-workspace-name");
  await expect(name).toBeVisible();
  await expect(name).toBeFocused();
  await expect(page.getByTestId("new-workspace-create")).toBeVisible();
});

test("escape collapses the create form back to the action", async ({ page }) => {
  await page.getByTestId("new-workspace").click();
  await expect(page.getByTestId("new-workspace-name")).toBeVisible();

  await page.getByTestId("new-workspace-name").press("Escape");
  await expect(page.getByTestId("new-workspace-name")).toBeHidden();
  await expect(page.getByTestId("new-workspace")).toBeVisible();
});
