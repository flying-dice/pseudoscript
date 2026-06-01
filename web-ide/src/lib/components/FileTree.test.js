import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import FileTree from "./FileTree.svelte";

const files = [
  { fqn: "orders", path: "orders.pds" },
  { fqn: "shared", path: "shared.pds" },
];

// The file tree is the entry point every e2e flow drives, so its rows must be
// addressable by a stable `data-testid` (file-<fqn>), never by label text.
describe("FileTree", () => {
  it("renders a stable data-testid per module", () => {
    render(FileTree, { props: { files } });
    expect(screen.getByTestId("file-orders")).toBeInTheDocument();
    expect(screen.getByTestId("file-shared")).toBeInTheDocument();
  });

  it("shows the module fqn as the row label", () => {
    render(FileTree, { props: { files } });
    expect(screen.getByTestId("file-orders")).toHaveTextContent("orders");
  });

  it("calls onopen with the file when a row is clicked", async () => {
    const onopen = vi.fn();
    render(FileTree, { props: { files, onopen } });
    await userEvent.click(screen.getByTestId("file-shared"));
    expect(onopen).toHaveBeenCalledTimes(1);
    expect(onopen).toHaveBeenCalledWith(files[1]);
  });

  it("marks the open file with aria-current", () => {
    render(FileTree, { props: { files, openPath: "orders.pds" } });
    expect(screen.getByTestId("file-orders")).toHaveAttribute("aria-current", "true");
    expect(screen.getByTestId("file-shared")).not.toHaveAttribute("aria-current");
  });
});
