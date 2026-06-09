import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Toast notifications: a workspace action raises one, and it can be dismissed by
// its close button. Creating a module is a reliable success-toast trigger.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("raises a toast on a workspace action and dismisses it", async ({ page }) => {
  await page.getByTestId("file-orders").click({ button: "right" });
  await page.getByRole("menuitem", { name: "New file…" }).click();
  const dialog = page.getByTestId("prompt-dialog");
  await dialog.getByRole("textbox").fill("noted");
  await dialog.getByRole("textbox").press("Enter");

  const toast = page.getByTestId("toast-success").filter({ hasText: "Created noted.pds" });
  await expect(toast).toBeVisible();
  await toast.getByTestId("toast-close").click();
  await expect(toast).toBeHidden();
});
