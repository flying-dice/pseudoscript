import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Bootstraps the ACME Tickets template into a real (OPFS) folder and opens its
// `orders` module before each test — the disk-backed path a user takes. All
// navigation is via data-testid; editor internals use CodeMirror's documented
// classes and our data-sem highlight attributes.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");
});

test("highlighting is AST-aware: tokens carry their LSP role", async ({ page }) => {
  // The token pass colours keywords; the AST pass colours node names as
  // namespaces — a declaration-context role a regex tokenizer could not assign.
  // (Param/field type tokens live inside folded bodies, so they are asserted in
  // the Rust unit tests rather than here.)
  await expect(page.locator('[data-sem="keyword"]').first()).toBeVisible();
  await expect(page.locator('[data-sem="namespace"]').first()).toBeVisible();
});

test("completion is scoped to a module path, not the full symbol set", async ({ page }) => {
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\nshared::User");

  const options = page.locator(".cm-tooltip-autocomplete li");
  // module `shared` has exactly one symbol matching the `User` prefix. The first
  // dropdown is wasm-backed and debounced, so allow for a loaded machine.
  await expect(options).toHaveCount(1, { timeout: 15_000 });
  await expect(options.first()).toContainText("UserId");
  // the general keyword set must not leak into a `::` context
  await expect(page.locator(".cm-tooltip-autocomplete li", { hasText: /^system$/ })).toHaveCount(0);
});

test("completion narrows as the prefix is typed (scope fix)", async ({ page }) => {
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\nshared::");
  // explicit trigger at the bare caret shows the whole module
  await page.keyboard.press("Control+Space");
  await expect(page.locator(".cm-tooltip-autocomplete li").first()).toBeVisible({ timeout: 15_000 });
  const all = await page.locator(".cm-tooltip-autocomplete li").count();
  expect(all).toBeGreaterThan(1);
  // typing a prefix narrows the same scoped set without leaking keywords
  await page.keyboard.type("User");
  await expect(page.locator(".cm-tooltip-autocomplete li")).toHaveCount(1, { timeout: 10_000 });
});

test("diagnostics render with no runtime error", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  // a stray token makes the module invalid → the linter must produce a marker
  await page.keyboard.type("\n%% broken");
  await expect(page.locator(".cm-lintRange, .cm-lint-marker").first()).toBeVisible({ timeout: 5000 });
  // the byte→char mapping in the linter must not throw (regressed once)
  expect(errors.join("\n")).not.toContain("byteToChar");
});

test("folding ranges come from the compiler (blocks fold)", async ({ page }) => {
  // The IDE folds blocks by default using the compiler's AST fold ranges; a
  // folded region renders CodeMirror's placeholder.
  await expect(page.locator(".cm-foldPlaceholder").first()).toBeVisible();

  // A fold must not swallow the lines between declarations: every fold starts
  // at its declaration header, so no rendered line jams two closing braces or
  // leaks a sibling's content (the regression when folds began at doc comments).
  // No jamming, and a folded brace sits flush against the placeholder — the
  // closing line's indentation folds away too (no `…   }` trailing space).
  const bad = await page.evaluate(() =>
    [...document.querySelectorAll(".cm-content .cm-line")]
      .map((l) => l.innerText)
      .filter((t) => /…\}…\}/.test(t) || /…\s+\}/.test(t)).length,
  );
  expect(bad).toBe(0);

  // The doc comment above a record stays visible; the record body folds to `{…}`.
  const docVisible = await page.evaluate(() =>
    [...document.querySelectorAll(".cm-content .cm-line")].some((l) =>
      l.innerText.startsWith("///"),
    ),
  );
  expect(docVisible).toBe(true);
});

test("the toolbar downloads the authoring skill folder as a zip", async ({ page }) => {
  const link = page.getByTestId("download-skill");
  await expect(link).toBeVisible();
  const [download] = await Promise.all([page.waitForEvent("download"), link.click()]);
  expect(download.suggestedFilename()).toBe("pseudocode-skill.zip");
});
