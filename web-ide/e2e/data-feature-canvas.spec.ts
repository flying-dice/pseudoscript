import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Opening a `data` node or a `feature` on the canvas used to crash the page
// (a feature is not a graph node, so its scene failed to lay out and the throw
// escaped). These pin the two new views (data ER, feature flow) rendering with
// no runtime error — the regression that prompted this work.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("selecting a data node draws its entity (ER) view, no crash", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));

  await page.getByLabel("Canvas").click();
  // Any data node projects an entity card with its fields.
  await page.locator(".node.kind-data").first().click();
  await expect(page.locator(".er-card").first()).toBeVisible({ timeout: 20_000 });
  expect(errors, errors.join("\n")).toHaveLength(0);
});

test("selecting a feature draws its step flow, no crash", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));

  await page.getByLabel("Canvas").click();
  // A feature is not a graph node; selecting it must project a flow, not throw.
  await page.locator(".node.kind-feature").first().click();
  await expect(page.locator(".feat-step").first()).toBeVisible({ timeout: 20_000 });
  expect(errors, errors.join("\n")).toHaveLength(0);
});
