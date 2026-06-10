import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// "Build docs" writes the same site `pds doc` ships into the opened folder's
// target/doc/ — disk only. The bundled in-memory examples get a notice telling
// them to open a folder (the old blob-preview path is gone). Selectors are
// data-testid / role only.

test.beforeEach(async ({ page }) => {
  await stubPicker(page);
});

test("build docs writes the site into the opened folder", async ({ page }) => {
  // createProject scaffolds to a real (OPFS) folder, so the workspace has a root.
  await createProject(page, "empty");

  await page.getByRole("button", { name: "File", exact: true }).click();
  await page.getByRole("menuitem", { name: "Build docs" }).click();

  // The success toast reports the write; the site lands under target/doc/.
  await expect(page.getByText("Documentation built")).toBeVisible({ timeout: 30_000 });
  const written = await page.evaluate(async () => {
    const root = window.__lastPicked;
    if (!root) return [];
    // The project scaffolds into a <name> subdirectory of the picked dir.
    const out: string[] = [];
    for await (const [name, handle] of root as unknown as AsyncIterable<[string, FileSystemHandle]>) {
      if (handle.kind !== "directory") continue;
      try {
        const target = await (handle as FileSystemDirectoryHandle).getDirectoryHandle("target");
        const doc = await target.getDirectoryHandle("doc");
        for await (const [file] of doc as unknown as AsyncIterable<[string, FileSystemHandle]>) {
          out.push(`${name}/target/doc/${file}`);
        }
      } catch {
        // not the project dir
      }
    }
    return out;
  });
  expect(written.some((p) => p.endsWith("/index.html"))).toBe(true);
  expect(written.some((p) => p.endsWith("/universe.html"))).toBe(true);
  expect(written.some((p) => p.endsWith("/health.html"))).toBe(true);
  expect(written.some((p) => p.endsWith("/search-index.js"))).toBe(true);
});

test("an in-memory example gets the open-a-folder notice, with no preview", async ({ page }) => {
  // Load a bundled example (in memory, no root).
  await page.goto("/");
  await page.getByTestId("example-acme-tickets").click({ timeout: 30_000 });
  await expect(page.getByTestId("editor").locator(".cm-content")).toBeVisible({ timeout: 20_000 });

  await page.getByRole("button", { name: "File", exact: true }).click();
  await page.getByRole("menuitem", { name: "Build docs" }).click();

  const notice = page.getByTestId("build-notice");
  await expect(notice).toBeVisible();
  await expect(notice.getByRole("button", { name: /preview/i })).toHaveCount(0);
  await expect(notice.getByRole("button", { name: "Open a folder…" })).toBeVisible();
  await notice.getByRole("button", { name: "Cancel" }).click();
  await expect(notice).toHaveCount(0);
});
