import { expect, test } from "@playwright/test";

// Opens the ACME Tickets sample and its `orders` module before each test. All
// navigation is via data-testid; editor internals use CodeMirror's documented
// classes and our data-sem highlight attributes.
test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("sample-acme-tickets").click();
  await page.getByTestId("file-orders").click();
  const content = page.getByTestId("editor").locator(".cm-content");
  await expect(content).toBeVisible();
  // wait until the compiler has produced highlight marks (wasm is ready)
  await expect(page.locator('[data-sem="keyword"]').first()).toBeVisible();
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
  // module `shared` has exactly one symbol matching the `User` prefix
  await expect(options).toHaveCount(1);
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
  await expect(page.locator(".cm-tooltip-autocomplete li").first()).toBeVisible();
  const all = await page.locator(".cm-tooltip-autocomplete li").count();
  expect(all).toBeGreaterThan(1);
  // typing a prefix narrows the same scoped set without leaking keywords
  await page.keyboard.type("User");
  await expect(page.locator(".cm-tooltip-autocomplete li")).toHaveCount(1);
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
