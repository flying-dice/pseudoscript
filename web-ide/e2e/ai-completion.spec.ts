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
    {
      enabled: true,
      provider: "custom",
      baseUrl: PROVIDER,
      apiKey: "",
      model: "test-model",
      mode: "chat",
      ...overrides,
    },
  );
}

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "*",
};

// Fulfil the provider's completion route (and its CORS preflight) with a canned
// insertion.
async function mockProvider(page: Page, completion: string, status = 200): Promise<void> {
  await page.route(`${PROVIDER}/chat/completions`, async (route) => {
    if (route.request().method() === "OPTIONS") {
      await route.fulfill({ status: 204, headers: CORS });
      return;
    }
    await route.fulfill({
      status,
      headers: { ...CORS, "Content-Type": "application/json" },
      body: JSON.stringify({ choices: [{ message: { content: completion } }] }),
    });
  });
}

// Move the caret to the end of the document. Control+End is the binding on
// Linux/Windows but a no-op in Chromium on macOS (where it is Cmd+ArrowDown);
// press both — each is inert on the other platform — so the splice point the
// parse-gate assertions depend on is deterministic everywhere.
async function gotoDocEnd(page: Page): Promise<void> {
  await page.keyboard.press("Control+End");
  await page.keyboard.press("Meta+ArrowDown");
}

// Fulfil the provider's model-list route (what "Test connection" hits first).
async function mockModels(page: Page, ids: string[], status = 200): Promise<void> {
  await page.route(`${PROVIDER}/models`, async (route) => {
    if (route.request().method() === "OPTIONS") {
      await route.fulfill({ status: 204, headers: CORS });
      return;
    }
    await route.fulfill({
      status,
      headers: { ...CORS, "Content-Type": "application/json" },
      body: JSON.stringify({ data: ids.map((id) => ({ id })) }),
    });
  });
}

test("guided setup: provider presets, custom fields, and the status chip", async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");

  // No chip while the feature is off.
  await expect(page.getByTestId("llm-status")).toHaveCount(0);

  // The header gear opens Settings directly on the AI tab (#43).
  await page.getByTestId("settings-btn").click();
  await expect(page.getByTestId("settings-dialog")).toBeVisible();
  await expect(page.getByTestId("llm-panel")).toBeVisible();

  // Presets expose only what they need: Ollama (default) has no URL or key;
  // OpenAI adds the masked key field and pins its endpoint.
  await expect(page.getByTestId("llm-baseurl")).toHaveCount(0);
  await expect(page.getByTestId("llm-apikey")).toHaveCount(0);
  await page.getByTestId("llm-provider-openai").click();
  await expect(page.getByTestId("llm-apikey")).toHaveAttribute("type", "password");
  await expect(page.getByTestId("llm-baseurl")).toHaveCount(0);

  // Custom is the raw-fields escape hatch.
  await page.getByTestId("llm-provider-custom").click();
  await page.getByTestId("llm-enabled").check();
  await page.getByTestId("llm-baseurl").fill(PROVIDER);
  await page.getByTestId("llm-model").fill("test-model");
  await page.getByTestId("llm-mode").selectOption("chat");
  await page.getByTestId("settings-dialog").getByRole("button", { name: "Close" }).click();

  // The status-bar chip appears live and reopens settings on the AI tab —
  // its "click to configure" lands on the page it advertises.
  const chip = page.getByTestId("llm-status");
  await expect(chip).toBeVisible();
  await chip.click();
  await expect(page.getByTestId("settings-dialog")).toBeVisible();
  await expect(page.getByTestId("llm-panel")).toBeVisible();
});

test("Help → AI Completion… opens Settings on the AI tab (#43)", async ({ page }) => {
  await stubPicker(page);
  await createProject(page, "acme-tickets", "orders");

  await page.getByRole("button", { name: "Help" }).click();
  await page.getByRole("menuitem", { name: "AI Completion…" }).click();
  await expect(page.getByTestId("settings-dialog")).toBeVisible();
  await expect(page.getByTestId("llm-panel")).toBeVisible();
});

test("Test connection reports success, and a classified failure with its hint", async ({
  page,
}) => {
  await stubPicker(page);
  await seedLlm(page);
  await mockModels(page, ["test-model"]);
  await mockProvider(page, "ok");
  await createProject(page, "acme-tickets", "orders");

  await page.getByTestId("settings-btn").click();
  await page.getByTestId("llm-test").click();
  await expect(page.getByTestId("llm-test-result")).toContainText("Connected — 1 model(s)");

  // Re-route to an auth failure: the result flips to the classified error.
  await page.unroute(`${PROVIDER}/models`);
  await mockModels(page, [], 401);
  await page.getByTestId("llm-test").click();
  const result = page.getByTestId("llm-test-result");
  await expect(result).toContainText("rejected the API key");
  await expect(result).toContainText("API key");
});

test("ghost text appears after an idle pause and Tab accepts it", async ({ page }) => {
  await stubPicker(page);
  await seedLlm(page);
  await mockProvider(page, " }");
  await createProject(page, "acme-tickets", "orders");

  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await gotoDocEnd(page);
  // Ending on "{" deterministically closes the grammar dropdown (no identifier
  // prefix under the caret), so Tab/Escape reach the ghost. The key interplay
  // with an open popup is unit-covered (inline-completion.test.ts).
  await page.keyboard.type("\npublic system Zzq {");

  // Debounced fetch → parse-validated suggestion → greyed widget at the caret.
  const ghost = page.getByTestId("ghost-text");
  await expect(ghost).toBeVisible({ timeout: 15_000 });
  await expect(ghost).toContainText("}");
  await expect(page.locator(".cm-tooltip-autocomplete")).toHaveCount(0);

  await page.keyboard.press("Tab");
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);
  await expect(content).toContainText("public system Zzq { }");
});

test("Escape dismisses the suggestion without inserting", async ({ page }) => {
  await stubPicker(page);
  await seedLlm(page);
  await mockProvider(page, " }");
  await createProject(page, "acme-tickets", "orders");

  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await gotoDocEnd(page);
  await page.keyboard.type("\npublic system Zzq {");
  await expect(page.getByTestId("ghost-text")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".cm-tooltip-autocomplete")).toHaveCount(0);

  await page.keyboard.press("Escape");
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);
  await expect(content).not.toContainText("public system Zzq { }");
});

test("a failing provider surfaces on the chip and a toast, and never blocks typing", async ({
  page,
}) => {
  await stubPicker(page);
  await seedLlm(page);
  await createProject(page, "acme-tickets", "orders");

  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));

  // 500s from the provider: no ghost, but the failure is visible — the chip
  // flips to its error state and one toast points at it.
  await mockProvider(page, "", 500);
  const content = page.getByTestId("editor").locator(".cm-content");
  await content.click();
  await gotoDocEnd(page);
  await page.keyboard.type("\npublic system Pay");
  await expect(page.getByTestId("toast-error")).toContainText("AI completion failed", {
    timeout: 15_000,
  });
  await expect(page.getByTestId("llm-status")).toHaveAttribute("title", /AI completion failing/);
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);

  // Repeated failures of the same kind stay quiet — still exactly one toast.
  await page.keyboard.press("Backspace");
  await page.keyboard.type("y");
  await page.waitForTimeout(1200); // past debounce + response
  await expect(page.getByTestId("toast-error")).toHaveCount(1);

  // A syntactically-broken suggestion still renders — it is the author's to
  // judge (typing on dismisses it; accepting it lights the live diagnostics) —
  // and the successful round-trip clears the chip's error state.
  await page.unroute(`${PROVIDER}/chat/completions`);
  await mockProvider(page, "%% not pseudoscript {{");
  await page.keyboard.type("m");
  const ghost = page.getByTestId("ghost-text");
  await expect(ghost).toBeVisible({ timeout: 15_000 });
  await expect(ghost).toContainText("%% not pseudoscript {{");
  await expect(page.getByTestId("llm-status")).not.toHaveAttribute(
    "title",
    /AI completion failing/,
  );

  // Typing on clears the unwanted suggestion without inserting it.
  await page.keyboard.type("ents;");
  await expect(page.getByTestId("ghost-text")).toHaveCount(0);
  await expect(content).toContainText("public system Payments;");
  await expect(content).not.toContainText("%% not pseudoscript {{");
  expect(errors).toEqual([]);
});
