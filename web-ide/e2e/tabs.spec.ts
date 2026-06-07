import { expect, test, type Page } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The ACME Tickets template opens `backoffice` and the `Overview` doc on load;
// opening `orders`, then `catalog` and `shared`, gives a known five-tab order:
// [backoffice, Overview, orders, catalog, shared], with `shared` active.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
  await page.getByTestId("file-catalog").click();
  await page.getByTestId("file-shared").click();
  await expect(page.locator(".tabbar .tab")).toHaveCount(5);
});

const labels = (page: Page) => page.locator(".tabbar .tab .tab-label").allInnerTexts();

test("right-click → Close Others leaves only that tab, and it becomes active", async ({ page }) => {
  await page.locator(".tabbar .tab", { hasText: "catalog" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Close Others" }).click();

  await expect(page.locator(".tabbar .tab")).toHaveCount(1);
  expect(await labels(page)).toEqual(["catalog"]);
  await expect(page.locator(".tab.active .tab-label")).toHaveText("catalog");
});

test("right-click → Close to the Right drops only later tabs", async ({ page }) => {
  await page.locator(".tabbar .tab", { hasText: "orders" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Close to the Right" }).click();

  // catalog and shared (to the right of orders) are closed; the rest stay.
  expect(await labels(page)).toEqual(["backoffice", "Overview", "orders"]);
});

test("right-click → Close All empties the tab bar", async ({ page }) => {
  await page.locator(".tabbar .tab").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Close All" }).click();

  await expect(page.locator(".tabbar .tab")).toHaveCount(0);
});

test("drag reorders tabs without changing the active file", async ({ page }) => {
  // `shared` is active after the beforeEach; drag it to the front.
  await expect(page.locator(".tab.active .tab-label")).toHaveText("shared");
  const shared = page.locator(".tabbar .tab", { hasText: "shared" });
  const backoffice = page.locator(".tabbar .tab", { hasText: "backoffice" });
  await shared.dragTo(backoffice);

  expect(await labels(page)).toEqual(["shared", "backoffice", "Overview", "orders", "catalog"]);
  // Reorder never changes which file is open.
  await expect(page.locator(".tab.active .tab-label")).toHaveText("shared");
});
