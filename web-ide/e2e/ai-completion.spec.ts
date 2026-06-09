import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { createProject, stubPicker } from "./harness";

// Inline AI ghost text against a mocked OpenAI-compatible provider: the route
// answers what a chat/completions endpoint would, so the whole client path —
// settings, debounce, request shape, parse-validation, the ghost widget, and
// Tab-accept — runs for real with no network.

const PROVIDER = "https://llm.e2e.test/v1";

// Seed persisted AI settings before the app boots (the store reads localStorage
// at module load, the same instant a returning user's settings apply).
async function seedLlm(page: Page, overrides: Record<string, unknown> = {}): Promise<void> {
  await page.addInitScript(
    (settings) => localStorage.setItem("pds.llm", JSON.stringify(settings)),
    { enabled: true, baseUrl: PROVIDER, apiKey: "", model: "test-model", mode: "chat", ...overrides },
  );
}

// Fulfil the provider route (and its CORS preflight) with a canned insertion.
async function mockProvider(page: Page, completion: string, status = 200): Promise<void> {
  await page.route(`${PROVIDER}/chat/completions`, async (route) => {
    const cors = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Headers": "*",
      "Access-Control-Allow-Methods": "*",
    };
    if (route.request().method() === "OPTIONS") {
      await route.fulfill({ status: 204, headers: cors });
      return;
    }
    await route.fulfill({
      status,
      headers: { ...cors, "Content-Type": "application/json" },
      body: JSON.stringify({ choices: [{ message: { content: completion } }] }),
    });
  });
}

test("AI Completion settings tab configures the store and shows the status chip", async ({
  page,
}) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");

  // No chip while the feature is off.
  await expect(page.getByTestId("llm-status")).toHaveCount(0);

  await page.keyboard.press("Control+k");
  await page.getByTestId("command-input").fill("shortcuts");
  await page.getByTestId("cmd-action-settings").click();
  await expect(page.getByTestId("settings-dialog")).toBeVisible();

  await page.getByTestId("settings-tab-ai").click();
  await page.getByTestId("llm-enabled").check();
  await page.getByTestId("llm-baseurl").fill(PROVIDER);
  await page.getByTestId("llm-model").fill("test-model");
  await page.getByTestId("llm-mode").selectOption("chat");
  // The key field is masked.
  await expect(page.getByTestId("llm-apikey")).toHaveAttribute("type", "password");
  await page.getByTestId("settings-dialog").getByRole("button", { name: "Close" }).click();

  // The status-bar chip appears live and reopens settings on click.
  const chip = page.getByTestId("llm-status");
  await expect(chip).toBeVisible();
  await chip.click();
  await expect(page.getByTestId("settings-dialog")).toBeVisible();
});

test("ghost text appears after an idle pause and Tab accepts it", async ({ page }) => {
  await stubPicker(page);
  await seedLlm(page);
  await mockProvider(page, "uake;");
  await createProject(page, "acme-tickets", "orders");

  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\npublic system Zzq");

  // Debounced fetch → parse-validated suggestion → greyed widget at the caret.
  // The grammar dropdown opens beside it (the engine indexes the just-typed
  // declaration); both visible at once is the Copilot-style steady state.
  const ghost = page.getByTestId("ghost-text");
  await expect(ghost).toBeVisible({ timeout: 15_000 });
  await expect(ghost).toHaveText("uake;");

  // With the popup open the dropdown owns Tab/Escape — first Escape closes it
  // and leaves the ghost standing.
  await expect(page.locator(".cm-tooltip-autocomplete")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(".cm-tooltip-autocomplete")).toHaveCount(0);
  await expect(ghost).toBeVisible();

  await page.keyboard.press("Tab");
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);
  await expect(content).toContainText("public system Zzquake;");
});

test("Escape dismisses the suggestion without inserting", async ({ page }) => {
  await stubPicker(page);
  await seedLlm(page);
  await mockProvider(page, "uake;");
  await createProject(page, "acme-tickets", "orders");

  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\npublic system Zzq");
  await expect(page.getByTestId("ghost-text")).toBeVisible({ timeout: 15_000 });

  // First Escape closes the dropdown popup; the second reaches the ghost.
  await expect(page.locator(".cm-tooltip-autocomplete")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(".cm-tooltip-autocomplete")).toHaveCount(0);
  await expect(page.getByTestId("ghost-text")).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);
  await expect(content).not.toContainText("Zzquake;");
});

test("a failing provider never blocks typing and a broken suggestion never shows", async ({
  page,
}) => {
  await stubPicker(page);
  await seedLlm(page);
  await createProject(page, "acme-tickets", "orders");

  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));

  // 500s from the provider: swallowed, typing unaffected.
  await mockProvider(page, "", 500);
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\npublic system Pay");
  await page.waitForTimeout(1200); // past debounce + response
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);

  // A syntactically-broken suggestion is dropped by the wasm parse gate.
  await page.unroute(`${PROVIDER}/chat/completions`);
  await mockProvider(page, "%% not pseudoscript {{");
  await page.keyboard.type("m");
  await page.waitForTimeout(1200);
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);

  await page.keyboard.type("ents;");
  await expect(content).toContainText("public system Payments;");
  expect(errors).toEqual([]);
});
