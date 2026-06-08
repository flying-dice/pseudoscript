import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import "@testing-library/jest-dom/vitest";

import StructurePanel from "./StructurePanel.svelte";

const symbols = [
  { fqn: "orders", name: "Orders", kind: "container" },
  { fqn: "orders::Place", name: "Place", kind: "callable", parent: "orders" },
  { fqn: "shared", name: "Shared", kind: "data" },
];

describe("StructurePanel", () => {
  it("renders the symbol tree under the filter", () => {
    render(StructurePanel, { props: { symbols } });
    expect(screen.getByTestId("structure-panel")).toBeInTheDocument();
    expect(screen.getByTestId("symbol-orders")).toBeInTheDocument();
  });

  it("filters to matches while keeping ancestors", async () => {
    render(StructurePanel, { props: { symbols } });
    await fireEvent.input(screen.getByTestId("structure-filter"), { target: { value: "place" } });
    expect(screen.getByTestId("symbol-orders::Place")).toBeInTheDocument();
    expect(screen.getByTestId("symbol-orders")).toBeInTheDocument(); // ancestor kept
    expect(screen.queryByTestId("symbol-shared")).not.toBeInTheDocument();
  });

  it("shows the no-match state and clears the filter", async () => {
    render(StructurePanel, { props: { symbols } });
    await fireEvent.input(screen.getByTestId("structure-filter"), { target: { value: "zzz" } });
    expect(screen.getByTestId("structure-no-match")).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("structure-filter-clear"));
    expect(screen.getByTestId("symbol-shared")).toBeInTheDocument();
  });
});
