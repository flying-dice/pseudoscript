import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The architectural-principle lints (PDS-ARCH-*) surface as warnings in the
// problems dock, each with its violation code rendered as a link to the article
// explaining the principle. The acme-tickets sample carries a module dependency
// cycle (PDS-ARCH-002), so the warning appears on load without any edit.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("renders an architectural warning code as a link to its principle article", async ({ page }) => {
  await page.getByTestId("view-problems").click();
  await expect(page.getByTestId("bottom-dock")).toBeVisible();

  // The cycle warning's code is a link into docs/principles. Wait for the wasm
  // check to settle and the link to appear.
  const link = page
    .getByTestId("problems-pane")
    .locator('a.code[href*="docs/principles/PDS-ARCH-002"]')
    .first();
  await expect(link).toBeVisible({ timeout: 15_000 });
  await expect(link).toHaveText("PDS-ARCH-002");
  await expect(link).toHaveAttribute("target", "_blank");
  await expect(link).toHaveAttribute(
    "href",
    "https://github.com/flying-dice/pseudoscript/blob/main/docs/principles/PDS-ARCH-002-cyclic-dependency.md",
  );
});
