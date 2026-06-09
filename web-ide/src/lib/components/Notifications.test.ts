import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import Notifications from "./Notifications.svelte";

const notes = [
  { id: 1, kind: "success" as const, title: "Saved" },
  { id: 2, kind: "error" as const, title: "Build failed", body: "missing module" },
];

describe("Notifications", () => {
  it("renders each toast title and body", () => {
    render(Notifications, { props: { notes } });
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(screen.getByText("Build failed")).toBeInTheDocument();
    expect(screen.getByText("missing module")).toBeInTheDocument();
  });

  it("dismisses a toast by id via its close button", async () => {
    const ondismiss = vi.fn();
    render(Notifications, { props: { notes, ondismiss } });
    const [first] = screen.getAllByRole("button", { name: "Dismiss notification" });
    await userEvent.click(first);
    expect(ondismiss).toHaveBeenCalledWith(1);
  });

  it("renders nothing for an empty stack", () => {
    render(Notifications, { props: { notes: [] } });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
