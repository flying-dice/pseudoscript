import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import PromptDialog from "./PromptDialog.svelte";

describe("PromptDialog", () => {
  it("renders the title and labelled input", () => {
    render(PromptDialog, { props: { title: "New file", label: "Name" } });
    expect(screen.getByTestId("prompt-dialog")).toHaveTextContent("New file");
    expect(screen.getByLabelText("Name")).toBeInTheDocument();
  });

  it("confirms the trimmed value", async () => {
    const onconfirm = vi.fn();
    render(PromptDialog, { props: { title: "New file", confirmLabel: "Create", onconfirm } });
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "  orders.pds  " } });
    await userEvent.click(screen.getByRole("button", { name: "Create" }));
    expect(onconfirm).toHaveBeenCalledWith("orders.pds");
  });

  it("blocks submit and shows the validation error", async () => {
    const onconfirm = vi.fn();
    const validate = (v: string) => (v === "taken" ? "Already exists" : null);
    render(PromptDialog, { props: { title: "New file", confirmLabel: "Create", validate, onconfirm } });
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "taken" } });
    expect(screen.getByText("Already exists")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Create" }));
    expect(onconfirm).not.toHaveBeenCalled();
  });

  it("cancels", async () => {
    const oncancel = vi.fn();
    render(PromptDialog, { props: { title: "New file", oncancel } });
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(oncancel).toHaveBeenCalled();
  });
});
