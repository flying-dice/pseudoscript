import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import TabBar from "./TabBar.svelte";

type Tab = { key: string; label: string; kind: "module" | "doc" | "manifest"; active: boolean; dirty: boolean };

const tabs: Tab[] = [
  { key: "orders", label: "orders.pds", kind: "module", active: true, dirty: false },
  { key: "docs/intro.md", label: "Intro", kind: "doc", active: false, dirty: true },
];

describe("TabBar", () => {
  it("selects a tab on click", async () => {
    const onselect = vi.fn<(key: string) => void>();
    render(TabBar, { props: { tabs, onselect } });
    await userEvent.click(screen.getByRole("tab", { name: /Intro/ }));
    expect(onselect).toHaveBeenCalledWith("docs/intro.md");
  });

  it("closes a tab via its close button", async () => {
    const onclose = vi.fn<(key: string) => void>();
    render(TabBar, { props: { tabs, onclose } });
    await userEvent.click(screen.getByRole("button", { name: "Close orders.pds" }));
    expect(onclose).toHaveBeenCalledWith("orders");
  });

  it("closes a tab on middle-click", async () => {
    const onclose = vi.fn<(key: string) => void>();
    render(TabBar, { props: { tabs, onclose } });
    // Middle-click fires `auxclick` with button 1 on the tab row, not the close button.
    const row = screen.getByRole("tab", { name: /Intro/ }).parentElement as HTMLElement;
    await fireEvent(row, new MouseEvent("auxclick", { button: 1, bubbles: true }));
    expect(onclose).toHaveBeenCalledWith("docs/intro.md");
  });
});
