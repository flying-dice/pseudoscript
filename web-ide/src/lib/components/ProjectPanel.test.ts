import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import type { Recent } from "$lib/recents.js";
import ProjectPanel from "./ProjectPanel.svelte";

const recents: Recent[] = [
  { key: "folder:payments", kind: "folder", name: "Payments", dir: "payments", at: Date.now() - 60_000 },
  { key: "folder:tickets", kind: "folder", name: "Tickets", dir: "tickets", at: Date.now() - 3_600_000 },
  { key: "sample:banking", kind: "sample", name: "Banking", sampleId: "banking", at: Date.now() },
];

describe("ProjectPanel", () => {
  it("lists folder recents only (samples are templates, not re-openable)", () => {
    render(ProjectPanel, { props: { recents } });
    expect(screen.getByTestId("recent-folder:payments")).toBeInTheDocument();
    expect(screen.getByTestId("recent-folder:tickets")).toBeInTheDocument();
    expect(screen.queryByTestId("recent-sample:banking")).not.toBeInTheDocument();
  });

  it("filters recents as you type", async () => {
    render(ProjectPanel, { props: { recents } });
    await fireEvent.input(screen.getByTestId("search-projects"), { target: { value: "tick" } });
    expect(screen.queryByTestId("recent-folder:payments")).not.toBeInTheDocument();
    expect(screen.getByTestId("recent-folder:tickets")).toBeInTheDocument();
  });

  it("fires the open, new, pick and forget callbacks", async () => {
    const onopenfolder = vi.fn();
    const onnewproject = vi.fn();
    const onpickrecent = vi.fn();
    const onforget = vi.fn();
    render(ProjectPanel, { props: { recents, onopenfolder, onnewproject, onpickrecent, onforget } });

    await userEvent.click(screen.getByTestId("open-folder"));
    expect(onopenfolder).toHaveBeenCalled();
    await userEvent.click(screen.getByTestId("new-project"));
    expect(onnewproject).toHaveBeenCalled();
    await userEvent.click(screen.getByTestId("recent-folder:payments"));
    expect(onpickrecent).toHaveBeenCalledWith(recents[0]);
    await userEvent.click(screen.getByRole("button", { name: "Remove Payments from recent" }));
    expect(onforget).toHaveBeenCalledWith(recents[0]);
  });

  it("shows the empty hint when there are no folder recents", () => {
    render(ProjectPanel, { props: { recents: [] } });
    expect(screen.getByText(/No recent projects yet/)).toBeInTheDocument();
  });

  it("lists the bundled examples and opens one in memory", async () => {
    const onpickexample = vi.fn();
    const examples = [
      { id: "banking", name: "Internet Banking", description: "C4 example.", moduleCount: 1 },
      { id: "acme-pay", name: "ACME Pay", description: "Payments.", moduleCount: 16 },
    ];
    render(ProjectPanel, { props: { recents: [], examples, onpickexample } });

    expect(screen.getByText(/Examples — open in your browser/)).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("example-banking"));
    expect(onpickexample).toHaveBeenCalledWith("banking");
  });

  it("disables the disk actions and explains, with examples still open, when FS Access is missing", () => {
    const examples = [{ id: "banking", name: "Internet Banking", description: "C4 example.", moduleCount: 1 }];
    render(ProjectPanel, { props: { recents: [], examples, fsSupported: false } });

    expect(screen.getByTestId("open-folder")).toBeDisabled();
    expect(screen.getByTestId("new-project")).toBeDisabled();
    expect(screen.getByTestId("fs-note")).toHaveTextContent(/examples below still open/i);
    expect(screen.getByTestId("example-banking")).toBeEnabled();
  });
});
