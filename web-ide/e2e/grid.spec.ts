import { type Page, expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Grid placement: enabling it unlocks drag-to-pin. Unlocking pins nothing — only a card
// you drag is pinned (the rest keep flowing). A pinned card shows a pin badge; clicking
// it clears that node's pin; the Reset control drops the whole diagram's placements back
// to the auto-layout. (The grid geometry is covered by the Rust grid_layout tests.)
//
// One serial journey on a shared page: the expensive setup (OPFS scaffold, mount, wasm
// check, canvas render, grid search) runs ONCE instead of per test. Four independent
// tests each paid that ~20s boot and, under suite parallelism, rode the 30s timeout on
// slower machines — the suite's one chronic flake. The steps are inherently ordered
// (unlock → pin → clear → re-pin → reset), so serial loses nothing.
test.describe.configure({ mode: "serial" });

let page: Page;

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
  await stubPicker(page);
  // Banking's context view has four cards to pin (a person + three systems) and
  // scaffolds a single module — the cheapest fixture that exercises the feature.
  // Its landing is Markdown, so skip the highlight gate.
  await createProject(page, "banking", undefined, { highlight: false });
  await page.getByLabel("Canvas").click();
  await expect(page.locator(".svelte-flow__node-card").nth(1)).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await page?.close();
});

// Drag one card a short hop and drop it, snapping it to a grid cell → one pin. A
// svelte-flow node drag needs a manual stepped mouse gesture to trip `onnodedragstop`.
async function pinFirstCard(): Promise<void> {
  const card = page.locator(".svelte-flow__node-card").first();
  const box = await card.boundingBox();
  if (!box) throw new Error("no card to drag");
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  await page.mouse.move(cx + 40, cy + 30, { steps: 8 });
  await page.mouse.up();
}

test("unlocking arms editing but pins nothing", async () => {
  // Turn on grid placement via the Layout dropdown, then unlock drag-to-pin.
  await page.getByRole("button", { name: "Layout" }).click();
  await page.getByRole("menuitemcheckbox", { name: "Grid placement" }).click();
  await page.keyboard.press("Escape"); // close the dropdown
  const lock = page.getByTestId("grid-lock");
  await expect(lock).toBeVisible({ timeout: 20_000 });
  await lock.click(); // arm drag-to-pin — pins nothing yet

  // No node is pinned until the user drags one.
  await expect(page.getByTestId("pin-badge")).toHaveCount(0);
});

test("dragging a card pins just that card", async () => {
  await pinFirstCard();

  const badges = page.getByTestId("pin-badge");
  await expect(badges).toHaveCount(1, { timeout: 20_000 });
});

test("clicking the pin badge clears a single node's pin", async () => {
  const badges = page.getByTestId("pin-badge");
  await expect(badges).toHaveCount(1);

  await badges.first().click();
  await expect(badges).toHaveCount(0, { timeout: 20_000 });
});

test("Reset drops the whole diagram's placements", async () => {
  await pinFirstCard();
  await expect(page.getByTestId("pin-badge").first()).toBeVisible({ timeout: 20_000 });

  const reset = page.getByTestId("grid-reset");
  await expect(reset).toBeVisible();
  await reset.click();
  await expect(page.getByTestId("pin-badge")).toHaveCount(0, { timeout: 20_000 });
  // With no pins left, the Reset control hides itself.
  await expect(reset).toHaveCount(0);
});
