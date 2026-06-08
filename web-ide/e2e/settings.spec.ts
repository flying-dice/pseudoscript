import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The keyboard-shortcuts settings: open, rebind a command by recording a chord,
// reject a conflicting chord, and reset. Opened from the palette's action.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
  await page.keyboard.press("Control+k");
  await page.getByTestId("command-input").fill("shortcuts");
  await page.getByTestId("cmd-action-settings").click();
  await expect(page.getByTestId("settings-dialog")).toBeVisible();
});

test("rebinds a command by recording a fresh chord", async ({ page }) => {
  const chip = page.getByTestId("keybind-duplicateLine");
  await chip.click();
  await expect(chip).toContainText("Press keys");
  await page.keyboard.press("Control+Alt+j");
  await expect(chip).toContainText("J");
  // The reset for that row is now enabled (it is custom).
  await expect(page.getByTestId("keybind-reset-duplicateLine")).toBeEnabled();
});

test("warns instead of rebinding to an in-use chord", async ({ page }) => {
  // Park a reproducible chord on one command, then try to reuse it on another.
  await page.getByTestId("keybind-findUsages").click();
  await page.keyboard.press("Control+Alt+j");
  await page.getByTestId("keybind-duplicateLine").click();
  await page.keyboard.press("Control+Alt+j");
  await expect(page.getByTestId("keybind-conflict")).toContainText("already used");
});

test("reset all clears every customisation", async ({ page }) => {
  await page.getByTestId("keybind-saveDocument").click();
  await page.keyboard.press("Control+Alt+m");
  await expect(page.getByTestId("keybind-reset-saveDocument")).toBeEnabled();
  await page.getByTestId("settings-reset-all").click();
  await expect(page.getByTestId("keybind-reset-saveDocument")).toBeDisabled();
});
