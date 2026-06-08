import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The hash router puts the live session in the URL: a folder-backed project carries
// a `#/f.<folder>/<view>?…` route, and a refresh restores where you were (the folder
// reopens from its persisted OPFS handle — same-origin, no prompt).
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("writes the folder route and active file into the hash", async ({ page }) => {
  await createProject(page, "acme-tickets", "orders");
  await expect.poll(() => page.url()).toContain("#/f.my-architecture/code");
  await expect.poll(() => page.url()).toContain("f=orders");
});

test("restores the active file after a reload", async ({ page }) => {
  await createProject(page, "acme-tickets", "orders");
  await page.getByTestId("file-payments").click();
  await expect.poll(() => page.url()).toContain("f=payments");

  await page.reload();

  // Reopened from disk, mounted, and re-landed on the same module.
  await expect(page.getByTestId("file-payments")).toHaveAttribute("aria-current", "true", { timeout: 20_000 });
});

test("restores the canvas view after a reload", async ({ page }) => {
  await createProject(page, "acme-tickets", "orders");
  await page.getByTestId("view-canvas").click();
  await expect.poll(() => page.url()).toContain("/canvas");

  await page.reload();

  await expect(page.getByTestId("canvas-view")).toBeVisible({ timeout: 20_000 });
});

test("restores the 3D universe focus after a reload", async ({ page }) => {
  // Banking's structural nodes have stable FQNs to target the 3D view with.
  await createProject(page, "banking", undefined, { highlight: false });
  await expect(page.getByTestId("symbol-banking::Customer")).toBeVisible({ timeout: 20_000 });

  // Focus a node in the universe from the structure panel.
  await page.getByTestId("symbol-banking::StaticContent").click({ button: "right" });
  await page.getByTestId("ctx-show-universe").click();
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });
  await expect.poll(() => page.url()).toContain("/space");
  await expect.poll(() => page.url()).toContain("n=banking%3A%3AStaticContent");

  await page.reload();

  // Restored into the universe still targeting the same node (applySpaceTarget runs,
  // so spaceFocus is set and ForceGraph flies to it on mount).
  await expect(page.getByTestId("universe")).toBeVisible({ timeout: 20_000 });
  await expect.poll(() => page.url()).toContain("n=banking%3A%3AStaticContent");
});
