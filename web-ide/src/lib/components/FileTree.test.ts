import type { ComponentProps } from "svelte";

import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

// Pull in the jest-dom matcher type augmentation (toBeInTheDocument,
// toHaveAttribute, …); the runtime registration lives in vitest-setup.js.
import "@testing-library/jest-dom/vitest";

import FileTree from "./FileTree.svelte";

// Shape of a file row the tree renders and reports back via `onopen`.
export interface TreeFile {
  fqn: string;
  path: string;
}

// FileTree declares every action callback as an optional prop, but Svelte's
// generated props type marks them required. Tests only pass the few props a
// case needs, so render through a partial-props wrapper.
type FileTreeProps = ComponentProps<typeof FileTree>;

function renderTree(props: Partial<FileTreeProps>): void {
  render(FileTree, { props: props as FileTreeProps });
}

const files: TreeFile[] = [
  { fqn: "orders", path: "orders.pds" },
  { fqn: "shared", path: "shared.pds" },
];

// The file tree is the entry point every e2e flow drives, so its rows must be
// addressable by a stable `data-testid` (file-<fqn>), never by label text.
describe("FileTree", () => {
  it("renders a stable data-testid per module", () => {
    renderTree({ files });
    expect(screen.getByTestId("file-orders")).toBeInTheDocument();
    expect(screen.getByTestId("file-shared")).toBeInTheDocument();
  });

  it("shows the module fqn as the row label", () => {
    renderTree({ files });
    expect(screen.getByTestId("file-orders")).toHaveTextContent("orders");
  });

  it("calls onopen with the file when a row is clicked", async () => {
    const onopen = vi.fn<(file: TreeFile) => void>();
    renderTree({ files, onopen });
    await userEvent.click(screen.getByTestId("file-shared"));
    expect(onopen).toHaveBeenCalledTimes(1);
    expect(onopen).toHaveBeenCalledWith(files[1]);
  });

  it("marks the open file with aria-current", () => {
    renderTree({ files, openPath: "orders.pds" });
    expect(screen.getByTestId("file-orders")).toHaveAttribute("aria-current", "true");
    expect(screen.getByTestId("file-shared")).not.toHaveAttribute("aria-current");
  });
});
