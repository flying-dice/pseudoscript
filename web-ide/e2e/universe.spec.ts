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
