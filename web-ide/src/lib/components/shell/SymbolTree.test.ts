import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import SymbolTree from "./SymbolTree.svelte";

const symbols = [
  { fqn: "orders", name: "Orders", kind: "container" as const },
  { fqn: "orders::Place", name: "Place", kind: "callable" as const, parent: "orders" },
  { fqn: "shared", name: "Shared", kind: "data" as const },
];

describe("SymbolTree", () => {
  it("shows the empty state with no nodes", () => {
    render(SymbolTree, { props: { symbols: [] } });
    expect(screen.getByText("No nodes declared yet.")).toBeInTheDocument();
  });

  it("nests children under their structural parent (expanded by default)", () => {
    render(SymbolTree, { props: { symbols } });
    expect(screen.getByTestId("symbol-orders")).toBeInTheDocument();
    expect(screen.getByTestId("symbol-orders::Place")).toBeInTheDocument();
    expect(screen.getByTestId("symbol-shared")).toBeInTheDocument();
  });

  it("collapses a subtree via its twist", async () => {
    render(SymbolTree, { props: { symbols } });
    await userEvent.click(screen.getByTestId("twist-orders"));
    expect(screen.queryByTestId("symbol-orders::Place")).not.toBeInTheDocument();
  });

  it("calls onpicknode with the node fqn on click", async () => {
    const onpicknode = vi.fn();
    render(SymbolTree, { props: { symbols, onpicknode } });
    await userEvent.click(screen.getByTestId("symbol-orders::Place"));
    expect(onpicknode).toHaveBeenCalledWith("orders::Place");
  });

  it("marks the selected node active", () => {
    render(SymbolTree, { props: { symbols, selectedFqn: "shared" } });
    expect(screen.getByTestId("symbol-shared").className).toContain("active");
  });
});
