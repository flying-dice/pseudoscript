import { expect, test } from "@playwright/test";
import { stubPicker } from "./harness";

// The first-run / launcher UX fixes (#50–#54, #57): examples mount in memory
// with no disk, the launcher is dismissible, missing File System Access only
// disables the disk actions, a picker failure surfaces inline, the language
// reference opens in-product, and the perf HUD is a View-menu toggle.
// Selectors are data-testid only.

test("an example opens in memory from the launcher — no folder, no picker", async ({ page }) => {
  // Deliberately NO picker stub: the example path must never touch the picker.
  await page.goto("/");
  await page.getByTestId("example-banking").click({ timeout: 30_000 });
  // Banking lands on its Markdown overview (no .pds highlight) — gate on the
  // mounted editor, then open the module and wait for the wasm highlight.
  const content = page.getByTestId("editor").locator(".cm-content");
  await expect(content).toBeVisible({ timeout: 20_000 });
  await page.getByTestId("file-banking").click();
  await expect(page.locator('[data-sem="keyword"]').first()).toBeVisible({ timeout: 20_000 });
});

test("the launcher dismisses with Escape and reopens from the empty stage", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("new-project")).toBeVisible({ timeout: 30_000 });
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("new-project")).not.toBeVisible();
  // The empty stage offers the way back in.
  await page.getByText("open a project").click();
  await expect(page.getByTestId("new-project")).toBeVisible();
});

test("without File System Access the disk actions disable but examples still open", async ({ page }) => {
  await page.addInitScript(() => {
    // Simulate Firefox/Safari: no File System Access API at all.
    // @ts-expect-error -- removing a Chromium-only global
    delete window.showDirectoryPicker;
  });
  await page.goto("/");
  await expect(page.getByTestId("fs-note")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("open-folder")).toBeDisabled();
  await expect(page.getByTestId("new-project")).toBeDisabled();
  // The in-memory path stays fully alive.
  await page.getByTestId("example-banking").click();
  await expect(page.getByTestId("editor").locator(".cm-content")).toBeVisible({ timeout: 20_000 });
});

test("a folder-picker failure renders inline in the New Project dialog", async ({ page }) => {
  await page.addInitScript(() => {
    window.showDirectoryPicker = () =>
      Promise.reject(new DOMException("blocked by permissions policy", "SecurityError"));
  });
  await page.goto("/");
  await page.getByTestId("new-project").click({ timeout: 30_000 });
  await page.getByTestId("new-project-name").fill("probe");
  await page.getByTestId("choose-folder").click();
  await expect(page.getByTestId("pick-error")).toContainText("blocked by permissions policy");
});

test("Help opens the bundled language reference; Escape closes it", async ({ page }) => {
  await stubPicker(page);
  await page.goto("/");
  await expect(page.getByTestId("new-project")).toBeVisible({ timeout: 30_000 });
  await page.keyboard.press("Escape"); // the menus sit behind the launcher
  await page.getByRole("button", { name: "Help" }).click();
  await page.getByTestId("menu-reference").click();
  await expect(page.getByTestId("reference-doc")).toBeVisible();
  await expect(page.getByTestId("reference-doc")).toContainText("PseudoScript", { timeout: 10_000 });
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("reference-doc")).not.toBeVisible();
});

test("the performance meter is a View-menu toggle", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("new-project")).toBeVisible({ timeout: 30_000 });
  await page.keyboard.press("Escape");
  const meter = page.getByTestId("perf-meter");
  const initiallyOn = await meter.isVisible();
  await page.getByRole("button", { name: "View" }).click();
  await page.getByRole("menuitemcheckbox", { name: "Performance meter" }).click();
  if (initiallyOn) await expect(meter).not.toBeVisible();
  else await expect(meter).toBeVisible();
});
