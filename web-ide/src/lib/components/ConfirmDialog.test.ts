import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import ConfirmDialog from "./ConfirmDialog.svelte";

describe("ConfirmDialog", () => {
  it("shows the title, message and confirm label", () => {
    render(ConfirmDialog, { props: { title: "Delete file", message: "orders.pds?", confirmLabel: "Delete" } });
    const dialog = screen.getByTestId("confirm-dialog");
    expect(dialog).toHaveTextContent("Delete file");
    expect(dialog).toHaveTextContent("orders.pds?");
  });

  it("fires onconfirm when the confirm button is clicked", async () => {
    const onconfirm = vi.fn();
    render(ConfirmDialog, { props: { confirmLabel: "Delete", onconfirm } });
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(onconfirm).toHaveBeenCalledTimes(1);
  });

  it("fires oncancel when Cancel is clicked", async () => {
    const oncancel = vi.fn();
    render(ConfirmDialog, { props: { oncancel } });
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(oncancel).toHaveBeenCalled();
  });
});
