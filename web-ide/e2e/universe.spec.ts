import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The 3D "universe" view participates in navigation history alongside code and
// canvas: selections record entries, reveal/go-to-definition record the universe as
// the Back origin. Selectors are data-testid only.

const FLOW = "symbol-banking::Customer::UseInternetBanking"; // a triggered flow
const NODE = "symbol-banking::StaticContent"; // a structural node

test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

async function openUniverse(page: import("@playwright/test").Page) {
  // Gate readiness on the universe rendering (it needs wasm), not the editor's
  // semantic-highlight, which is a flakier warm-up signal.
  await createProject(page, "banking", undefined, { highlight: false });
  await page.getByTestId("view-space").click();
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });
}

test("back/forward step through universe flow and node selections", async ({ page }) => {
  await openUniverse(page);

  // Open a flow — the timeline names it.
  await page.getByTestId(FLOW).click();
  await expect(page.getByTestId("flow-name")).toHaveText("UseInternetBanking");

  // Focus a node — the flow timeline goes away.
  await page.getByTestId(NODE).click();
  await expect(page.getByTestId("flow-name")).toHaveCount(0);

  // Back restores the flow, staying in the universe (not code/canvas).
  await page.getByTestId("nav-back").click();
  await expect(page.getByTestId("universe")).toBeVisible();
  await expect(page.getByTestId("flow-name")).toHaveText("UseInternetBanking");

  // Forward returns to the node focus.
  await page.getByTestId("nav-forward").click();
  await expect(page.getByTestId("universe")).toBeVisible();
  await expect(page.getByTestId("flow-name")).toHaveCount(0);
});

test("reveal on canvas from the universe; Back returns to the universe", async ({ page }) => {
  await openUniverse(page);
  await page.getByTestId(FLOW).click();
  await expect(page.getByTestId("flow-name")).toHaveText("UseInternetBanking");

  // Right-click a node → Reveal on canvas → leaves the universe for the canvas.
  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-reveal-canvas").click();
  await expect(page.getByTestId("canvas-view")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("universe")).toHaveCount(0);

  // Back returns to the universe (NOT the editor), the flow restored.
  await page.getByTestId("nav-back").click();
  await expect(page.getByTestId("universe")).toBeVisible();
  await expect(page.getByTestId("flow-name")).toHaveText("UseInternetBanking");
});

test("show in 3D view from the structure panel opens the universe; Back returns to the source", async ({ page }) => {
  // Start outside the universe; left-click a symbol first to seed a code-view origin.
  await createProject(page, "banking", undefined, { highlight: false });
  await page.getByTestId(NODE).click();
  await expect(page.getByTestId("editor")).toBeVisible();

  // Right-click a structural node in the structure panel → Show in 3D view.
  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-show-universe").click();
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });

  // Back returns to the originating view (the editor), leaving the universe.
  await page.getByTestId("nav-back").click();
  await expect(page.getByTestId("universe")).toHaveCount(0);
  await expect(page.getByTestId("editor")).toBeVisible();
});

test("right-clicking a node sphere opens its context menu (the canvas's actions)", async ({ page }) => {
  await openUniverse(page);

  // Focus a node from the structure panel — the camera flies to it, centring it
  // on the canvas, which makes the sphere a deterministic right-click target.
  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-show-universe").click();
  await expect(page.getByTestId("universe")).toBeVisible();

  // Right-click the canvas centre once the fly-to settles on the node.
  const box = (await page.getByTestId("universe-canvas").boundingBox())!;
  const menu = page.locator(".ctx-menu");
  await expect(async () => {
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
    await expect(menu).toBeVisible({ timeout: 500 });
  }).toPass({ timeout: 15_000 });
  await expect(menu.getByRole("menuitem", { name: "Reveal on canvas" })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Find usages" })).toBeVisible();

  // Go to definition leaves the universe for the editor.
  await menu.getByRole("menuitem", { name: "Go to definition" }).click();
  await expect(page.getByTestId("editor").locator(".cm-content")).toBeVisible();
  await expect(page.getByTestId("universe")).toHaveCount(0);
});

test("go to definition from the universe opens the source", async ({ page }) => {
  await openUniverse(page);

  // Right-click a node → Go to definition → leaves the universe for the editor.
  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-goto-definition").click();
  await expect(page.getByTestId("editor")).toBeVisible();
  await expect(page.getByTestId("universe")).toHaveCount(0);

  // Back returns to the universe.
  await page.getByTestId("nav-back").click();
  await expect(page.getByTestId("universe")).toBeVisible();
});
