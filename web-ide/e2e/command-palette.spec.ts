import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The ⌘K / Ctrl-K "search everywhere" palette: open, search a file, navigate.
// All selection is via data-testid; the palette mounts in a portal.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("opens on Ctrl-K and focuses the search input", async ({ page }) => {
  await page.keyboard.press("Control+k");
  await expect(page.getByTestId("command-palette")).toBeVisible();
  await expect(page.getByTestId("command-input")).toBeFocused();
});

test("finds a file and opens it, then closes", async ({ page }) => {
  await page.keyboard.press("Control+k");
  await page.getByTestId("command-input").fill("orders");
  const item = page.getByTestId("cmd-file-orders");
  await expect(item).toBeVisible();
  await item.click();
  // The palette closes and the file becomes the active tree entry.
  await expect(page.getByTestId("command-palette")).toBeHidden();
  await expect(page.getByTestId("file-orders")).toHaveAttribute("aria-current", "true");
});

test("Tab cycles the search-scope tabs", async ({ page }) => {
  await page.keyboard.press("Control+k");
  await expect(page.getByTestId("command-mode-all")).toHaveAttribute("aria-selected", "true");
  await page.getByTestId("command-input").press("Tab");
  await expect(page.getByTestId("command-mode-types")).toHaveAttribute("aria-selected", "true");
});
