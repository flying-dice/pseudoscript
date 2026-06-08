import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import { keybindings } from "$lib/keybindings.svelte.js";
import Settings from "./Settings.svelte";

// Settings drives the keybindings singleton directly — reset it between tests.
beforeEach(() => {
  localStorage.clear();
  keybindings.setProfile("default");
  keybindings.resetAll();
});
afterEach(() => localStorage.clear());

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
});
