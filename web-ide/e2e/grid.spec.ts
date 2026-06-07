import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Grid placement: enabling it unlocks drag-to-pin, which freezes every node onto a
// cell. Pinned cards show a badge; the inline ✕ clears one; the Reset control drops
// the whole diagram's placements back to the auto-layout. (The symmetric-margin
// geometry is covered by the Rust grid_layout tests.)
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

// Open a context canvas with several cards to pin (acme-tickets has multiple
// top-level systems + persons; its landing is Markdown, so skip the highlight gate).
async function openMultiNodeCanvas(page: import("@playwright/test").Page): Promise<void> {
  await createProject(page, "acme-tickets", undefined, { highlight: false });
  await page.getByLabel("Canvas").click();
  await expect(page.locator(".svelte-flow__node-card").nth(1)).toBeVisible({ timeout: 20_000 });
}

// Turn on grid placement via the Layout dropdown, then unlock (freezes all nodes).
async function enableGridAndUnlock(page: import("@playwright/test").Page): Promise<void> {
  await page.getByRole("button", { name: "Layout" }).click();
  await page.getByRole("menuitemcheckbox", { name: "Grid placement" }).click();
  await page.keyboard.press("Escape"); // close the dropdown
  const lock = page.getByTestId("grid-lock");
  await expect(lock).toBeVisible({ timeout: 20_000 });
  await lock.click(); // freeze the current arrangement → every card pinned
}

test("unlocking grid mode pins every node and shows a badge", async ({ page }) => {
  await openMultiNodeCanvas(page);
  await enableGridAndUnlock(page);

  const badges = page.getByTestId("pin-badge");
  await expect(badges.first()).toBeVisible({ timeout: 20_000 });
  const cards = page.locator(".svelte-flow__node-card");
  // Freeze pins the whole arrangement, so a badge per card.
  expect(await badges.count()).toBe(await cards.count());
});

test("the inline ✕ clears a single node's pin", async ({ page }) => {
  await openMultiNodeCanvas(page);
  await enableGridAndUnlock(page);

  const badges = page.getByTestId("pin-badge");
  await expect(badges.first()).toBeVisible({ timeout: 20_000 });
  const before = await badges.count();
  expect(before).toBeGreaterThan(1);

  await page.getByTestId("unpin-btn").first().click();
  await expect(badges).toHaveCount(before - 1, { timeout: 20_000 });
});

test("Reset drops the whole diagram's placements", async ({ page }) => {
  await openMultiNodeCanvas(page);
  await enableGridAndUnlock(page);

  await expect(page.getByTestId("pin-badge").first()).toBeVisible({ timeout: 20_000 });
  const reset = page.getByTestId("grid-reset");
  await expect(reset).toBeVisible();

  await reset.click();
  await expect(page.getByTestId("pin-badge")).toHaveCount(0, { timeout: 20_000 });
  // With no pins left, the Reset control hides itself.
  await expect(reset).toHaveCount(0);
});
