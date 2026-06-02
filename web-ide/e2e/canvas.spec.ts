import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The C4 canvas is driven by a right-click context menu (hover + click are
// deprecated). This pins the menu's presence and its "Go to definition" action.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("right-clicking a canvas node opens its context menu; Go to definition opens the source", async ({ page }) => {
  await createProject(page, "empty");

  // Switch to the Canvas activity and wait for the C4 graph to lay out.
  await page.getByLabel("Canvas").click();
  const node = page.locator(".svelte-flow__node-card").first();
  await expect(node).toBeVisible({ timeout: 20_000 });

  // Right-click opens the node context menu; a plain hover shows no info card.
  await node.hover();
  await expect(page.locator(".canvas-pop.info")).toHaveCount(0);
  await node.click({ button: "right" });
  const menu = page.locator(".ctx-menu");
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Find usages" })).toBeVisible();

  // Go to definition leaves the canvas for the editor.
  await menu.getByRole("menuitem", { name: "Go to definition" }).click();
  await expect(page.getByTestId("editor").locator(".cm-content")).toBeVisible();
  await expect(page.locator(".svelte-flow")).toHaveCount(0);
});

test("the menu bar persists across views", async ({ page }) => {
  await createProject(page, "empty");

  // The menu bar stays put in both the code view and the canvas.
  await expect(page.getByRole("button", { name: "File", exact: true })).toBeVisible();
  await page.getByLabel("Canvas").click();
  await expect(page.locator(".svelte-flow__node-card").first()).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole("button", { name: "File", exact: true })).toBeVisible();
});
