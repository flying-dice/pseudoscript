import type { Meta, StoryObj } from "@storybook/svelte";
import FileTree from "./FileTree.svelte";

// A workspace file: its fully-qualified name and base-relative path.
export interface FileEntry {
  fqn: string;
  path: string;
}

// The workspace file tree — the primary navigation surface. Rows are addressed
// in tests by `data-testid="file-<fqn>"`.
const meta: Meta<typeof FileTree> = {
  title: "IDE/FileTree",
  component: FileTree,
  tags: ["autodocs"],
};

export default meta;

type Story = StoryObj<typeof FileTree>;

const files: FileEntry[] = [
  { fqn: "orders", path: "orders.pds" },
  { fqn: "shared", path: "shared.pds" },
  { fqn: "gateway", path: "gateway.pds" },
];

export const TwoModules: Story = {
  args: { workspaceName: "ACME Tickets", files, openPath: "orders.pds" },
};

export const WithUnsavedAndError: Story = {
  args: {
    workspaceName: "ACME Tickets",
    files,
    openPath: "orders.pds",
    dirtyPaths: new Set(["shared.pds"]),
    errorPaths: new Set(["gateway.pds"]),
  },
};

export const Empty: Story = {
  args: { workspaceName: "Empty", files: [] },
};
