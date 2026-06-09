import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Every action of the right-hand structure panel (the workspace symbol outline):
// filter, expand/collapse, the view-aware left-click pick, the three context-menu
// actions (go to definition, reveal on canvas, show in 3D view), and the rail
// toggle that hides/shows the panel. Selectors are data-testid only.

const PERSON = "symbol-banking::Customer"; // a root node, present once wasm is ready
const FLOW = "symbol-banking::Customer::UseInternetBanking"; // a triggered flow
const NODE = "symbol-banking::StaticContent"; // a structural container
const BACKEND = "symbol-banking::Backend"; // a container with component children
const CHILD = "symbol-banking::SigninApi"; // a component nested under Backend

test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

// Boot banking and wait for the outline to populate (wasm-ready signal), without
// depending on the editor's flakier semantic-highlight warm-up.
async function openBanking(page: import("@playwright/test").Page) {
  await createProject(page, "banking", undefined, { highlight: false });
  await expect(page.getByTestId("structure-panel")).toBeVisible();
  await expect(page.getByTestId(PERSON)).toBeVisible({ timeout: 20_000 });
}

test("filter narrows the tree, no-match message shows, clear restores", async ({ page }) => {
  await openBanking(page);

  // A query keeps matches (and their ancestors), drops the rest.
  await page.getByTestId("structure-filter").fill("Signin");
  await expect(page.getByTestId(CHILD)).toBeVisible();
  await expect(page.getByTestId(NODE)).toHaveCount(0);

  // Gibberish matches nothing — the empty-state message appears.
  await page.getByTestId("structure-filter").fill("zzzznotarealsymbol");
  await expect(page.getByTestId("structure-no-match")).toBeVisible();

  // Clearing the filter restores the full tree.
  await page.getByTestId("structure-filter-clear").click();
  await expect(page.getByTestId(NODE)).toBeVisible();
  await expect(page.getByTestId(CHILD)).toBeVisible();
});

test("expand/collapse hides and shows a node's children", async ({ page }) => {
  await openBanking(page);

  // A component nested under Backend is visible while the parent is expanded.
  await expect(page.getByTestId(CHILD)).toBeVisible();

  // Collapsing Backend hides its children.
  await page.getByTestId("twist-banking::Backend").click();
  await expect(page.getByTestId(CHILD)).toHaveCount(0);
  await expect(page.getByTestId(BACKEND)).toBeVisible(); // the parent stays

  // Expanding again brings them back.
  await page.getByTestId("twist-banking::Backend").click();
  await expect(page.getByTestId(CHILD)).toBeVisible();
});

test("left-click selects a node and opens its source", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId(NODE).click();
  // The picked node is marked active, and the editor shows the source.
  await expect(page.getByTestId(NODE)).toHaveClass(/active/);
  await expect(page.getByTestId("editor")).toBeVisible();
});

test("go to definition opens the source", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-goto-definition").click();
  await expect(page.getByTestId("editor")).toBeVisible();
  await expect(page.getByTestId(NODE)).toHaveClass(/active/);
});

test("reveal on canvas switches to the canvas view", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-reveal-canvas").click();
  await expect(page.getByTestId("canvas-view")).toBeVisible({ timeout: 20_000 });
});

test("show in 3D view opens the universe", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId(NODE).click({ button: "right" });
  await page.getByTestId("ctx-show-universe").click();
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });
});

test("left-click is view-aware: in the universe it re-targets the 3D view", async ({ page }) => {
  await openBanking(page);

  // Enter the universe, then pick a flow from the panel — the timeline names it.
  await page.getByTestId("view-space").click();
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });
  await page.getByTestId(FLOW).click();
  await expect(page.getByTestId("flow-name")).toHaveText("UseInternetBanking");
});

test("left-click is view-aware: in the canvas it stays on the canvas", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId("view-canvas").click();
  await expect(page.getByTestId("canvas-view")).toBeVisible({ timeout: 20_000 });
  // Picking a node drills the diagram scope rather than leaving for the editor.
  await page.getByTestId(BACKEND).click();
  await expect(page.getByTestId("canvas-view")).toBeVisible();
  await expect(page.getByTestId(BACKEND)).toHaveClass(/active/);
});

test("the rail toggle hides and shows the structure panel", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId("toggle-structure").click();
  await expect(page.getByTestId("structure-panel")).toHaveCount(0);

  await page.getByTestId("toggle-structure").click();
  await expect(page.getByTestId("structure-panel")).toBeVisible();
});

// --- selection following (issue #40) -----------------------------------------
// The panel follows the shared selection wherever it is made: a canvas click, a
// 3D pick, or plain caret movement in the editor. The 3D pick funnels through
// the same selectNode path the canvas test pins (WebGL nodes have no DOM hit
// targets to click directly).

test("picking a canvas node highlights it in the structure panel", async ({ page }) => {
  await openBanking(page);

  await page.getByTestId("view-canvas").click();
  await expect(page.getByTestId("canvas-view")).toBeVisible({ timeout: 20_000 });
  // A canvas pick is the context menu's drill action (left-click is a no-op on
  // the card itself); the drill lands the picked node in the shared selection.
  const card = page.locator(".svelte-flow__node-card", { hasText: "InternetBanking" }).first();
  await expect(card).toBeVisible({ timeout: 20_000 });
  await card.dispatchEvent("contextmenu");
  // The menu anchors at the dispatched event's origin; dispatch the item's
  // click too rather than depend on its on-screen position.
  await page.getByRole("menuitem", { name: "Open container diagram" }).dispatchEvent("click");
  await expect(page.getByTestId("symbol-banking::InternetBanking")).toHaveClass(/active/);
});

test("moving the editor caret onto a symbol highlights it in the panel", async ({ page }) => {
  await openBanking(page);

  // Open a module in the editor by picking any node first.
  await page.getByTestId(NODE).click();
  await expect(page.getByTestId("editor")).toBeVisible();

  // Click the SinglePageApp declaration — plain caret movement, no navigation.
  await page.getByTestId("editor").locator(".cm-content").getByText("SinglePageApp", { exact: true }).first().click();

  // The highlight follows (debounced resolve), without leaving the code view.
  await expect(page.getByTestId("symbol-banking::SinglePageApp")).toHaveClass(/active/, { timeout: 5_000 });
  await expect(page.getByTestId("editor")).toBeVisible();
});

test("the reveal button is disabled without a selection and re-reveals a collapsed row", async ({ page }) => {
  await openBanking(page);

  // Nothing selected yet — the button is disabled.
  await expect(page.getByTestId("structure-reveal")).toBeDisabled();

  // Select a nested component; its row is active and the button arms.
  await page.getByTestId(CHILD).click();
  await expect(page.getByTestId(CHILD)).toHaveClass(/active/);
  await expect(page.getByTestId("structure-reveal")).toBeEnabled();

  // Manually collapse the parent — the selected row disappears and stays
  // hidden (the auto-reveal fires only on selection *change*).
  await page.getByTestId("twist-banking::Backend").click();
  await expect(page.getByTestId(CHILD)).toHaveCount(0);

  // The reveal button force-expands the ancestors and shows the row again.
  await page.getByTestId("structure-reveal").click();
  await expect(page.getByTestId(CHILD)).toBeVisible();
  await expect(page.getByTestId(CHILD)).toHaveClass(/active/);
});

test("filtering out the selection disarms reveal; clearing the filter re-reveals", async ({ page }) => {
  await openBanking(page);

  // Select a nested component, then hide it twice over: collapse its parent,
  // then filter it out of the tree entirely.
  await page.getByTestId(CHILD).click();
  await expect(page.getByTestId(CHILD)).toHaveClass(/active/);
  await page.getByTestId("twist-banking::Backend").click();
  await expect(page.getByTestId(CHILD)).toHaveCount(0);
  await page.getByTestId("structure-filter").fill("Customer");

  // The selected row isn't renderable under this filter — reveal would no-op,
  // so it disarms.
  await expect(page.getByTestId("structure-reveal")).toBeDisabled();

  // Clearing the filter re-runs the auto-reveal: the collapsed ancestor expands
  // and the selected row is visible again without touching the button.
  await page.getByTestId("structure-filter-clear").click();
  await expect(page.getByTestId(CHILD)).toBeVisible();
  await expect(page.getByTestId(CHILD)).toHaveClass(/active/);
  await expect(page.getByTestId("structure-reveal")).toBeEnabled();
});
