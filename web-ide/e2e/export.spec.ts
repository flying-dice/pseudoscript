import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// The C4 canvas can be downloaded as a PNG or SVG image. This pins the export
// control's presence and that each format triggers a real browser download.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

for (const format of ["PNG", "SVG"] as const) {
  test(`downloading the canvas as ${format} triggers a file download`, async ({ page }) => {
    await createProject(page, "empty");

    // Switch to the Canvas activity and wait for the C4 graph to lay out.
    await page.getByLabel("Canvas").click();
    await expect(page.locator(".svelte-flow__node-card").first()).toBeVisible({ timeout: 20_000 });

    // Open the export menu and pick the format; the click should download a file.
    await page.getByRole("button", { name: "Download diagram" }).click();
    const download = page.waitForEvent("download");
    await page.getByRole("menuitem", { name: `${format} image` }).click();
    expect((await download).suggestedFilename()).toMatch(new RegExp(`\\.${format.toLowerCase()}$`));
  });
}
