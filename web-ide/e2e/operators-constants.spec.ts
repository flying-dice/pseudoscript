import { expect, test } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// LANG.md §3.6 / §7.5 (ADR-038, ADR-039) — a model using a top-level `constant`
// and comparison/boolean operators type-checks clean and renders in the IDE; a
// malformed constant surfaces as a problem. The banking sample declares
// `constant MAX_SESSION_MINUTES` and a `RefreshToken` rule built from operators.
test.beforeEach(async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "banking", "banking");
});

test("a constant + operator business rule type-checks and renders", async ({ page }) => {
  // The sample loads clean — the operator rule and constant raise no problem.
  await expect(page.getByTestId("problems-badge")).toBeHidden();

  // The grammar renders: the wasm tokeniser highlights keywords (incl. `constant`).
  await expect(page.locator('[data-sem="keyword"]').first()).toBeVisible();

  // A malformed constant (non-literal right-hand side) surfaces as a problem,
  // proving the constant checker runs in the IDE.
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\npublic constant BAD = notALiteral");
  await expect(page.getByTestId("problems-badge")).toBeVisible({ timeout: 15_000 });
});
