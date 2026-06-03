import { type Locator, type Page, expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The hit targets are SVG <tspan>s / canvas cards with no stable click point, so
// dispatch the contextmenu event the handler listens for instead of a pointer
// click. The target symbol comes from the element's bound data, not the pointer.
async function rightClick(locator: Locator): Promise<void> {
  await locator.dispatchEvent("contextmenu");
}

// The sequence diagram is driven by the same right-click context menu as the C4
// graph (the hold-hover info card and Cmd-click are gone). This pins the menu on
// a message label and on a lifeline card, plus its "Go to definition" action.
// One project load drives all three — the acme-tickets scaffold is heavy.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

// Open the acme-tickets checkout flow as a sequence: right-click the Attendee
// node on the canvas and pick its `checkout` entry point.
async function openCheckoutFlow(page: Page): Promise<void> {
  // acme-tickets lands on `docs/overview.md` (Markdown, no `.pds` highlight), so
  // skip the harness highlight gate; the canvas-node wait below gates wasm-ready.
  await createProject(page, "acme-tickets", undefined, { highlight: false });
  await page.getByLabel("Canvas").click();
  const attendee = page.locator(".svelte-flow__node-card", { hasText: "Attendee" }).first();
  await expect(attendee).toBeVisible({ timeout: 20_000 });
  await attendee.click({ button: "right" });
  await page.locator(".ctx-menu").getByRole("menuitem", { name: "checkout" }).click();
  await expect(page.locator(".seq-messages")).toBeVisible({ timeout: 20_000 });
}

test("right-click opens the actions menu on sequence messages and lifelines", async ({ page }) => {
  await openCheckoutFlow(page);
  const menu = page.locator(".ctx-menu");

  // No hover info card exists any more.
  await expect(page.locator(".canvas-pop.info")).toHaveCount(0);

  // A message label: both actions present.
  const label = page.locator(".seq-hit").first();
  await expect(label).toBeVisible();
  await rightClick(label);
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Find usages" })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Go to definition" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);

  // A lifeline card opens the same menu.
  const card = page.locator(".seq-card.interactive").first();
  await expect(card).toBeVisible();
  await rightClick(card);
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Go to definition" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);

  // A signature type token (a declared `data` type) opens the same menu.
  const typeLink = page.locator(".seq-type-link").first();
  await expect(typeLink).toBeVisible();
  await rightClick(typeLink);
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Go to definition" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);

  // Go to definition from a message leaves the canvas for the editor. The menu
  // anchors at the dispatched event's origin; dispatch the item's click too
  // rather than depend on its on-screen position.
  await rightClick(label);
  await menu.getByRole("menuitem", { name: "Go to definition" }).dispatchEvent("click");
  await expect(page.getByTestId("editor").locator(".cm-content")).toBeVisible();
  await expect(page.locator(".svelte-flow")).toHaveCount(0);
});
