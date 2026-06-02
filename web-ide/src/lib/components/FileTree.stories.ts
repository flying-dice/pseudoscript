import type { Meta, StoryObj } from "@storybook/svelte";

import FileTree from "./FileTree.svelte";

// A unified file-tree entry — module / doc / manifest, addressed in tests by
// `data-testid="file-<fqn>"`.
interface FileEntry {
  key: string;
  kind: "module" | "doc" | "manifest";
  relPath: string;
  label: string;
  fqn?: string;
}

const meta: Meta<typeof FileTree> = {
  title: "IDE/FileTree",
  component: FileTree,
  tags: ["autodocs"],
};

export default meta;

type Story = StoryObj<typeof FileTree>;

const entries: FileEntry[] = [
  { key: "orders", kind: "module", relPath: "orders.pds", label: "orders", fqn: "orders" },
  { key: "shared", kind: "module", relPath: "banking/shared.pds", label: "shared", fqn: "shared" },
  { key: "gateway", kind: "module", relPath: "banking/gateway.pds", label: "gateway", fqn: "gateway" },
  { key: "docs/intro.md", kind: "doc", relPath: "docs/intro.md", label: "Intro" },
  { key: "pds.toml", kind: "manifest", relPath: "pds.toml", label: "pds.toml" },
];

export const Workspace: Story = {
  args: { entries, openKey: "orders" },
};

export const WithUnsavedAndError: Story = {
  args: {
    entries,
    openKey: "orders",
    dirtyKeys: new Set(["shared"]),
    errorKeys: new Set(["gateway"]),
  },
};

export const Empty: Story = {
  args: { entries: [] },
};
