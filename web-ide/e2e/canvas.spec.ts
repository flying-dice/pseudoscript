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

test("clicking an edge opens the relationship menu, not the node behind it", async ({ page }) => {
  await createProject(page, "acme-tickets", "orders");

  await page.getByLabel("Canvas").click();
  await expect(page.locator(".svelte-flow__node-card").first()).toBeVisible({ timeout: 20_000 });

  // Click a labelled edge at its label anchor — the label is click-through, so
  // the click lands on the edge's hit-path beneath it (which sits above the
  // boundary frame, the old swallow-the-click bug).
  const label = page.locator(".svelte-flow__edge-label").first();
  await expect(label).toBeVisible();
  const box = (await label.boundingBox())!;
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);

  // The menu is the edge's (from → to), offering definition / usages per call.
  const menu = page.locator(".ctx-menu");
  await expect(menu).toBeVisible();
  await expect(menu.locator(".ctx-name")).toContainText("→");
  await expect(menu.getByRole("menuitem", { name: "Find usages" }).first()).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Go to definition" }).first()).toBeVisible();
});

test("the menu bar persists across views", async ({ page }) => {
  await createProject(page, "empty");

  // The menu bar stays put in both the code view and the canvas.
  await expect(page.getByRole("button", { name: "File", exact: true })).toBeVisible();
  await page.getByLabel("Canvas").click();
  await expect(page.locator(".svelte-flow__node-card").first()).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole("button", { name: "File", exact: true })).toBeVisible();
});

test("go to definition from the canvas, then Back returns to the canvas", async ({ page }) => {
  await createProject(page, "empty");

  await page.getByLabel("Canvas").click();
  const flow = page.locator(".svelte-flow");
  const node = page.locator(".svelte-flow__node-card").first();
  await expect(node).toBeVisible({ timeout: 20_000 });

  // Leave the canvas for the editor via Go to definition.
  await node.click({ button: "right" });
  await page.locator(".ctx-menu").getByRole("menuitem", { name: "Go to definition" }).click();
  await expect(page.getByTestId("editor").locator(".cm-content")).toBeVisible();
  await expect(flow).toHaveCount(0);

  // Back is a genuine back across the app — it returns to the canvas diagram we
  // were on, not the editor's last caret.
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await expect(flow).toBeVisible();
  await expect(page.getByTestId("editor").locator(".cm-content")).not.toBeVisible();
});

test("back/forward stays on the canvas across drill navigations", async ({ page }) => {
  await createProject(page, "empty");

  // On the canvas, drill the system into its container diagram, then the
  // container into its component diagram — two recorded canvas navigations.
  await page.getByLabel("Canvas").click();
  const flow = page.locator(".svelte-flow");
  const node = page.locator(".svelte-flow__node-card").first();
  await expect(node).toBeVisible({ timeout: 20_000 });

  await node.click({ button: "right" });
  await page.locator(".ctx-menu").getByRole("menuitem", { name: "Open container diagram" }).click();
  const container = page.locator(".svelte-flow__node-card").first();
  await expect(container).toBeVisible();
  await container.click({ button: "right" });
  await page.locator(".ctx-menu").getByRole("menuitem", { name: "Open component diagram" }).click();
  await expect(flow).toBeVisible();

  // Back returns to the previous diagram scope — staying on the canvas, not the
  // editor (the bug: back used to drop you into the code).
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await expect(flow).toBeVisible();
  await expect(page.getByTestId("editor").locator(".cm-content")).not.toBeVisible();

  // Forward re-applies the next scope, still on the canvas.
  await page.getByRole("button", { name: "Forward", exact: true }).click();
  await expect(flow).toBeVisible();
  await expect(page.getByTestId("editor").locator(".cm-content")).not.toBeVisible();
});
