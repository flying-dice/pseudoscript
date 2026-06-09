import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The light/dark toggle, driven from the command palette's "Switch theme" action.
// The resolved theme is applied as <html data-theme> and persisted to localStorage,
// so an inline head script re-applies it before first paint on the next load.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("toggles the theme and persists it across a reload", async ({ page }) => {
  const html = page.locator("html");
  const before = await html.getAttribute("data-theme");

  await page.keyboard.press("Control+k");
  await page.getByTestId("command-input").fill("theme");
  await page.getByTestId("cmd-action-theme").click();

  const after = before === "dark" ? "light" : "dark";
  await expect(html).toHaveAttribute("data-theme", after);
  // The address-bar theme-color meta flips with it.
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute(
    "content",
    after === "light" ? "#e7eaef" : "#0a0b0e",
  );

  await page.reload();
  await expect(html).toHaveAttribute("data-theme", after);
});
