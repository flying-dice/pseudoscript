import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import RenameDialog from "./RenameDialog.svelte";

const occurrences = [
  { fqn: "orders", line: 3, col: 10, text: "public Place(): void", match_start: 7, match_end: 12, decl: true },
  { fqn: "orders", line: 9, col: 4, text: "self.Place()", match_start: 5, match_end: 10, decl: false },
] as never;

describe("RenameDialog", () => {
  it("renders the symbol and the occurrence count", () => {
    render(RenameDialog, { props: { symbol: "Place", occurrences } });
    const dialog = screen.getByTestId("rename-dialog");
    expect(dialog).toHaveTextContent("Place");
    expect(screen.getByText("2 of 2 selected")).toBeInTheDocument();
  });

  it("confirms the new name with every selected occurrence", async () => {
    const onconfirm = vi.fn();
    render(RenameDialog, { props: { symbol: "Place", occurrences, onconfirm } });
    await fireEvent.input(screen.getByLabelText("New name"), { target: { value: "Reserve" } });
    await userEvent.click(screen.getByRole("button", { name: /Rename 2 occurrences/ }));
    expect(onconfirm).toHaveBeenCalledWith("Reserve", [
      { fqn: "orders", line: 3, col: 10 },
      { fqn: "orders", line: 9, col: 4 },
    ]);
  });

  it("drops a deselected occurrence from the rename", async () => {
    const onconfirm = vi.fn();
    render(RenameDialog, { props: { symbol: "Place", occurrences, onconfirm } });
    await fireEvent.input(screen.getByLabelText("New name"), { target: { value: "Reserve" } });
    // Uncheck the second occurrence (checkboxes: [select-all, occ1, occ2]).
    const boxes = screen.getAllByRole("checkbox");
    await userEvent.click(boxes[2]);
    expect(screen.getByText("1 of 2 selected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Rename 1 occurrence/ }));
    expect(onconfirm).toHaveBeenCalledWith("Reserve", [{ fqn: "orders", line: 3, col: 10 }]);
  });

  it("disables apply for an invalid identifier", async () => {
    render(RenameDialog, { props: { symbol: "Place", occurrences } });
    await fireEvent.input(screen.getByLabelText("New name"), { target: { value: "9bad" } });
    expect(screen.getByText("Not a valid identifier.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Rename/ })).toBeDisabled();
  });
});
