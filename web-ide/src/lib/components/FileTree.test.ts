import type { ComponentProps } from "svelte";

import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

// Pull in the jest-dom matcher type augmentation (toBeInTheDocument,
// toHaveAttribute, …); the runtime registration lives in vitest-setup.js.
import "@testing-library/jest-dom/vitest";

import FileTree from "./FileTree.svelte";

// A unified file-tree entry — module / doc / manifest.
interface FileEntry {
  key: string;
  kind: "module" | "doc" | "manifest" | "other";
  relPath: string;
  label: string;
  fqn?: string;
  binary?: boolean;
}

// FileTree declares every action callback as an optional prop, but Svelte's
// generated props type marks them required. Tests only pass the few props a
// case needs, so render through a partial-props wrapper.
type FileTreeProps = ComponentProps<typeof FileTree>;

function renderTree(props: Partial<FileTreeProps>): void {
  render(FileTree, { props: props as FileTreeProps });
}

const entries: FileEntry[] = [
  { key: "orders", kind: "module", relPath: "orders.pds", label: "orders", fqn: "orders" },
  { key: "shared", kind: "module", relPath: "banking/shared.pds", label: "shared", fqn: "shared" },
  { key: "docs/intro.md", kind: "doc", relPath: "docs/intro.md", label: "Intro" },
];

// The file tree is the entry point every e2e flow drives, so its module rows must
// be addressable by a stable `data-testid` (file-<fqn>), never by label text.
describe("FileTree", () => {
  it("renders a stable data-testid per module", () => {
    renderTree({ entries });
    expect(screen.getByTestId("file-orders")).toBeInTheDocument();
    expect(screen.getByTestId("file-shared")).toBeInTheDocument();
  });

  it("shows the file name as the row label and nests by directory", () => {
    renderTree({ entries });
    expect(screen.getByTestId("file-shared")).toHaveTextContent("shared.pds");
    // a directory folder is rendered for the nested module + the docs
    expect(screen.getByText("banking")).toBeInTheDocument();
    expect(screen.getByText("docs")).toBeInTheDocument();
  });

  it("calls onopen with the entry when a row is clicked", async () => {
    const onopen = vi.fn<(e: FileEntry) => void>();
    renderTree({ entries, onopen });
    await userEvent.click(screen.getByTestId("file-orders"));
    expect(onopen).toHaveBeenCalledTimes(1);
    expect(onopen).toHaveBeenCalledWith(entries[0]);
  });

  it("marks the open file with aria-current via openKey", () => {
    renderTree({ entries, openKey: "orders" });
    expect(screen.getByTestId("file-orders")).toHaveAttribute("aria-current", "true");
    expect(screen.getByTestId("file-shared")).not.toHaveAttribute("aria-current");
  });
});
