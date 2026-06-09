import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import { keybindings } from "$lib/keybindings.svelte.js";
import { llm } from "$lib/llm.svelte.js";
import Settings from "./Settings.svelte";

// Settings drives the keybindings and llm singletons directly — reset between
// tests. The AI tab fetches the provider's model list on open; stub fetch so no
// test touches the network (the rejecting default exercises the fallbacks).
const fetchMock = vi.fn();
beforeEach(() => {
  localStorage.clear();
  keybindings.setProfile("default");
  keybindings.resetAll();
  llm.reset();
  fetchMock.mockReset();
  fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("Settings", () => {
  it("renders the shortcuts dialog with command rows", () => {
    render(Settings, { props: { onclose: vi.fn() } });
    expect(screen.getByTestId("settings-dialog")).toBeInTheDocument();
    expect(screen.getByTestId("keybind-saveDocument")).toBeInTheDocument();
  });

  it("rebinds a command by recording a fresh chord", async () => {
    render(Settings, { props: { onclose: vi.fn() } });
    await userEvent.click(screen.getByTestId("keybind-duplicateLine"));
    expect(screen.getByTestId("keybind-duplicateLine")).toHaveTextContent("Press keys…");
    // Capture-phase window listener rebinds on the first non-modifier key.
    await fireEvent.keyDown(window, { key: "j", ctrlKey: true, altKey: true });
    expect(keybindings.keyFor("duplicateLine")).toBe("Ctrl-Alt-j");
    expect(keybindings.isCustom("duplicateLine")).toBe(true);
  });

  it("warns on a conflicting chord instead of rebinding", async () => {
    // Park a reproducible chord on another command, then try to reuse it.
    keybindings.setKey("openSearch", "Ctrl-Alt-j");
    render(Settings, { props: { onclose: vi.fn() } });
    await userEvent.click(screen.getByTestId("keybind-duplicateLine"));
    await fireEvent.keyDown(window, { key: "j", ctrlKey: true, altKey: true });
    expect(screen.getByTestId("keybind-conflict")).toHaveTextContent(/already used/);
    expect(keybindings.isCustom("duplicateLine")).toBe(false);
  });

  it("resets a single binding and all bindings", async () => {
    render(Settings, { props: { onclose: vi.fn() } });
    keybindings.setKey("saveDocument", "Mod-Alt-s");
    await userEvent.click(screen.getByTestId("keybind-reset-saveDocument"));
    expect(keybindings.isCustom("saveDocument")).toBe(false);

    keybindings.setKey("openSearch", "Mod-Alt-f");
    await userEvent.click(screen.getByTestId("settings-reset-all"));
    expect(keybindings.isCustom("openSearch")).toBe(false);
  });

  it("switches the keymap profile", async () => {
    render(Settings, { props: { onclose: vi.fn() } });
    await userEvent.selectOptions(screen.getByLabelText("Keymap"), "vscode");
    expect(keybindings.profile).toBe("vscode");
  });

  it("closes via the close button", async () => {
    const onclose = vi.fn();
    render(Settings, { props: { onclose } });
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalled();
  });

  it("opens on the keyboard tab and switches to AI Completion", async () => {
    render(Settings, { props: { onclose: vi.fn() } });
    expect(screen.getByTestId("keybind-saveDocument")).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("settings-tab-ai"));
    expect(screen.getByTestId("llm-panel")).toBeInTheDocument();
    expect(screen.queryByTestId("keybind-saveDocument")).not.toBeInTheDocument();
    await userEvent.click(screen.getByTestId("settings-tab-keyboard"));
    expect(screen.getByTestId("keybind-saveDocument")).toBeInTheDocument();
  });

  it("opens directly on the AI tab when the caller seeds initialTab", () => {
    render(Settings, { props: { onclose: vi.fn(), initialTab: "ai" } });
    expect(screen.getByTestId("llm-panel")).toBeInTheDocument();
    expect(screen.queryByTestId("keybind-saveDocument")).not.toBeInTheDocument();
  });

  it("picks a provider preset and shows only its fields", async () => {
    render(Settings, { props: { onclose: vi.fn(), initialTab: "ai" } });

    // Ollama (the default): no endpoint or key to type.
    expect(screen.queryByTestId("llm-baseurl")).not.toBeInTheDocument();
    expect(screen.queryByTestId("llm-apikey")).not.toBeInTheDocument();

    // OpenAI: the key appears, the endpoint stays pinned.
    await userEvent.click(screen.getByTestId("llm-provider-openai"));
    expect(llm.provider).toBe("openai");
    expect(llm.baseUrl).toBe("https://api.openai.com/v1");
    expect(screen.queryByTestId("llm-baseurl")).not.toBeInTheDocument();
    await userEvent.type(screen.getByTestId("llm-apikey"), "sk-test");
    expect(llm.apiKey).toBe("sk-test");
    expect(screen.getByTestId("llm-apikey")).toHaveAttribute("type", "password");
  });

  it("falls back to a static OpenAI model list when the live list can't load", async () => {
    render(Settings, { props: { onclose: vi.fn(), initialTab: "ai" } });
    await userEvent.click(screen.getByTestId("llm-provider-openai"));
    await userEvent.selectOptions(screen.getByTestId("llm-model"), "gpt-4o");
    expect(llm.model).toBe("gpt-4o");
  });

  it("edits the raw fields under the Custom preset", async () => {
    render(Settings, { props: { onclose: vi.fn() } });
    await userEvent.click(screen.getByTestId("settings-tab-ai"));

    await userEvent.click(screen.getByTestId("llm-enabled"));
    expect(llm.enabled).toBe(true);

    await userEvent.click(screen.getByTestId("llm-provider-custom"));
    const url = screen.getByTestId("llm-baseurl");
    await userEvent.clear(url);
    await userEvent.type(url, "https://api.example.test/v1");
    expect(llm.baseUrl).toBe("https://api.example.test/v1");

    await userEvent.type(screen.getByTestId("llm-apikey"), "sk-test");
    expect(llm.apiKey).toBe("sk-test");

    await userEvent.selectOptions(screen.getByTestId("llm-mode"), "fim");
    expect(llm.mode).toBe("fim");
  });

  it("Test connection reports a classified failure with its hint", async () => {
    render(Settings, { props: { onclose: vi.fn(), initialTab: "ai" } });
    await userEvent.click(screen.getByTestId("llm-test"));
    const result = await screen.findByTestId("llm-test-result");
    expect(result).toHaveTextContent("Could not reach");
    expect(result).toHaveTextContent("OLLAMA_ORIGINS");
  });

  it("Test connection reports success with the model count", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: () =>
        Promise.resolve({
          data: [{ id: "qwen2.5-coder:7b" }],
          choices: [{ message: { content: "ok" } }],
        }),
    });
    render(Settings, { props: { onclose: vi.fn(), initialTab: "ai" } });
    await userEvent.click(screen.getByTestId("llm-test"));
    const result = await screen.findByTestId("llm-test-result");
    expect(result).toHaveTextContent("Connected — 1 model(s) available.");
  });

  it("Test connection fills the OpenAI dropdown with chat models only", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: () =>
        Promise.resolve({
          data: [{ id: "gpt-4o-mini" }, { id: "whisper-1" }, { id: "text-embedding-3-small" }],
          choices: [{ message: { content: "ok" } }],
        }),
    });
    render(Settings, { props: { onclose: vi.fn(), initialTab: "ai" } });
    await userEvent.click(screen.getByTestId("llm-provider-openai"));
    await userEvent.click(screen.getByTestId("llm-test"));
    await screen.findByTestId("llm-test-result");
    const options = [...screen.getByTestId<HTMLSelectElement>("llm-model").options].map((o) => o.value);
    expect(options).toContain("gpt-4o-mini");
    expect(options).not.toContain("whisper-1");
    expect(options).not.toContain("text-embedding-3-small");
  });
});
