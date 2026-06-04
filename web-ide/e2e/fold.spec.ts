import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Code folding defaults: every callable (member) impl block collapses on open,
// while the structural bodies (person/system/container/component, data records)
// stay expanded. Right-click menu items fold/unfold. The empty starter keeps its
// one member near the top so the fold paints within the editor viewport.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("opening a file folds member impl blocks; the structure stays expanded", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  const body = content.getByText("Describe what happens here", { exact: false });

  // The container body stays expanded (its declaration is visible); the member
  // impl block is collapsed (its body hidden behind a `…` placeholder).
  await expect(content.getByText("container Api", { exact: false })).toBeVisible();
  await expect(content.locator(".cm-foldPlaceholder").first()).toBeVisible();
  await expect(body).toHaveCount(0);

  // The fold starts at the callable header (`Health(): void {…}`): the `///` doc
  // comment above it stays visible, and no rendered line jams two braces or
  // leaves a `…   }` trailing gap.
  const docVisible = await page.evaluate(() =>
    [...document.querySelectorAll(".cm-content .cm-line")].some((l) => (l as HTMLElement).innerText.startsWith("///")),
  );
  expect(docVisible).toBe(true);
  const bad = await page.evaluate(() =>
    [...document.querySelectorAll(".cm-content .cm-line")]
      .map((l) => (l as HTMLElement).innerText)
      .filter((t) => /…\}…\}/.test(t) || /…\s+\}/.test(t)).length,
  );
  expect(bad).toBe(0);
});

test("the right-click menu folds and unfolds", async ({ page }) => {
  await createProject(page, "empty");
  const content = page.getByTestId("editor").locator(".cm-content");
  const menu = page.locator(".cm-ctx");
  const body = content.getByText("Describe what happens here", { exact: false });

  // Unfold all reveals the collapsed member body.
  await content.locator('[data-sem="keyword"]').first().click({ button: "right" });
  await menu.getByRole("menuitem", { name: "Unfold all" }).click();
  await expect(body).toBeVisible();

  // Fold all members collapses it again.
  await content.locator('[data-sem="keyword"]').first().click({ button: "right" });
  await menu.getByRole("menuitem", { name: "Fold all members" }).click();
  await expect(body).toHaveCount(0);
});
