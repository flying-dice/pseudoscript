import { expect, test } from "@playwright/test";

// The IDE is an installable PWA: a linked web manifest, platform icons, and a
// theme colour. (The service worker is emitted by the build and registered by
// SvelteKit; install/offline behaviour isn't exercised against the dev server.)
test("ships an installable web app manifest with icons", async ({ page }) => {
  await page.goto("/");

  // The manifest is linked in the document head.
  await expect(page.locator('link[rel="manifest"]')).toHaveAttribute("href", /manifest\.webmanifest/);

  // It is valid, standalone, and declares the required + maskable icons.
  const manifest = await (await page.request.get("/manifest.webmanifest")).json();
  expect(manifest.name).toBe("PseudoScript IDE");
  expect(manifest.display).toBe("standalone");
  const sizes = manifest.icons.map((i: { sizes: string }) => i.sizes);
  expect(sizes).toContain("192x192");
  expect(sizes).toContain("512x512");
  expect(manifest.icons.some((i: { purpose?: string }) => i.purpose === "maskable")).toBe(true);

  // An icon actually resolves as a PNG.
  const icon = await page.request.get("/icons/icon-512.png");
  expect(icon.ok()).toBe(true);
  expect(icon.headers()["content-type"]).toContain("image/png");

  // The address-bar / splash theme colour is set.
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#0a0b0e");
});
