import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The gutters between the islands carry drag handles (Splitter) that resize the
// explorer / structure panels. This pins that a drag widens the panel, that the
// width clamps to its bounds, and that the choice persists to localStorage.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("dragging the explorer gutter resizes the panel, clamps, and persists", async ({ page }) => {
  await createProject(page, "empty");

  const splitter = page.getByRole("separator", { name: "Resize explorer" });
  await expect(splitter).toBeVisible();

  // Drag the handle right; the explorer widens past its 248px default.
  const box = await splitter.boundingBox();
  if (!box) throw new Error("splitter has no bounding box");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2, { steps: 6 });
  await page.mouse.up();
  expect(await page.evaluate(() => Number(localStorage.getItem("pds-explorer-w")))).toBeGreaterThan(248);

  // Arrow keys nudge the focused handle; shrinking well past the minimum clamps to 180.
  await splitter.focus();
  for (let i = 0; i < 50; i++) await splitter.press("ArrowLeft");
  expect(await page.evaluate(() => localStorage.getItem("pds-explorer-w"))).toBe("180");
});
