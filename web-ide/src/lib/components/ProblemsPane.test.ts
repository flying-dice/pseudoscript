import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import ProblemsPane from "./ProblemsPane.svelte";

const diagnostics = [
  { severity: "error", message: "unknown symbol", start_line: 3, start_col: 5, file: "orders", code: "PDS-RES-001" },
  { severity: "warning", message: "unused", start_line: 8, start_col: 1, file: "orders" },
];

describe("ProblemsPane", () => {
  it("shows the well-formed empty state with no diagnostics", () => {
    render(ProblemsPane, { props: { diagnostics: [] } });
    expect(screen.getByText(/No problems/)).toBeInTheDocument();
  });

  it("lists each problem with its count", () => {
    render(ProblemsPane, { props: { diagnostics } });
    expect(screen.getByText("2 problems")).toBeInTheDocument();
    expect(screen.getByText("unknown symbol")).toBeInTheDocument();
    expect(screen.getByText("unused")).toBeInTheDocument();
  });

  it("calls onpick with the problem when its row is clicked", async () => {
    const onpick = vi.fn();
    render(ProblemsPane, { props: { diagnostics, onpick } });
    await userEvent.click(screen.getByText("unknown symbol"));
    expect(onpick).toHaveBeenCalledWith(diagnostics[0]);
  });

  it("copies all problems as formatted text", async () => {
    const oncopy = vi.fn();
    render(ProblemsPane, { props: { diagnostics, oncopy } });
    await userEvent.click(screen.getByRole("button", { name: /Copy all/ }));
    expect(oncopy).toHaveBeenCalledTimes(1);
    const [text, count] = oncopy.mock.calls[0];
    expect(count).toBe(2);
    expect(text).toContain("error orders:3:5 unknown symbol [PDS-RES-001]");
    expect(text).toContain("warning orders:8:1 unused");
  });

  it("renders an architectural code with code_description as a link to the article", () => {
    const href = "https://github.com/flying-dice/pseudoscript/blob/main/docs/principles/PDS-ARCH-001-backdooring-facade.md";
    render(ProblemsPane, {
      props: {
        diagnostics: [
          {
            severity: "warning",
            message: "cross-module call reaches into internal component",
            start_line: 4,
            start_col: 7,
            file: "gateway",
            code: "PDS-ARCH-001",
            code_description: href,
          },
        ],
      },
    });
    const link = screen.getByTestId("problem-0-code");
    expect(link.tagName).toBe("A");
    expect(link).toHaveAttribute("href", href);
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveTextContent("PDS-ARCH-001");
  });

  it("renders a code with no code_description as plain text, not a link", () => {
    render(ProblemsPane, {
      props: {
        diagnostics: [
          { severity: "error", message: "unknown symbol", start_line: 1, start_col: 1, code: "PDS-RES-001" },
        ],
      },
    });
    const chip = screen.getByTestId("problem-0-code");
    expect(chip.tagName).toBe("SPAN");
    expect(chip).toHaveTextContent("PDS-RES-001");
  });
});
