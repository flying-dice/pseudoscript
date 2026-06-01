import FileTree from "./FileTree.svelte";

// The workspace file tree — the primary navigation surface. Rows are addressed
// in tests by `data-testid="file-<fqn>"`.
export default {
  title: "IDE/FileTree",
  component: FileTree,
  tags: ["autodocs"],
};

const files = [
  { fqn: "orders", path: "orders.pds" },
  { fqn: "shared", path: "shared.pds" },
  { fqn: "gateway", path: "gateway.pds" },
];

export const TwoModules = {
  args: { workspaceName: "ACME Tickets", files, openPath: "orders.pds" },
};

export const WithUnsavedAndError = {
  args: {
    workspaceName: "ACME Tickets",
    files,
    openPath: "orders.pds",
    dirtyPaths: new Set(["shared.pds"]),
    errorPaths: new Set(["gateway.pds"]),
  },
};

export const Empty = {
  args: { workspaceName: "Empty", files: [] },
};
