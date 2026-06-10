import { fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import NewProjectDialog from "./NewProjectDialog.svelte";
import type { PickOutcome } from "$lib/workspace.js";

const templates = [
  { id: "empty", name: "Empty", description: "A one-module starter.", moduleCount: 1 },
  { id: "banking", name: "Banking", description: "Worked example.", moduleCount: 7 },
];

describe("NewProjectDialog", () => {
  it("disables every template until a name and folder are set", () => {
    render(NewProjectDialog, { props: { templates } });
    expect(screen.getByTestId("new-project-name")).toBeInTheDocument();
    expect(screen.getByTestId("template-empty")).toBeDisabled();
    expect(screen.getByTestId("template-banking")).toBeDisabled();
  });

  it("enables templates once name + folder are set and scaffolds the chosen one", async () => {
    const folder = { name: "workspace" } as FileSystemDirectoryHandle;
    const onchoosefolder = vi.fn(async (): Promise<PickOutcome> => ({ kind: "picked", handle: folder }));
    const onpick = vi.fn();
    render(NewProjectDialog, { props: { templates, onchoosefolder, onpick } });

    await fireEvent.input(screen.getByTestId("new-project-name"), { target: { value: "  payments  " } });
    await userEvent.click(screen.getByTestId("choose-folder"));
    expect(onchoosefolder).toHaveBeenCalled();

    const card = screen.getByTestId("template-empty");
    expect(card).toBeEnabled();
    await userEvent.click(card);
    expect(onpick).toHaveBeenCalledWith("payments", "empty", folder);
  });

  it("keeps the prior state on a cancelled pick, silently", async () => {
    const onchoosefolder = vi.fn(async (): Promise<PickOutcome> => ({ kind: "cancelled" }));
    render(NewProjectDialog, { props: { templates, onchoosefolder } });

    await fireEvent.input(screen.getByTestId("new-project-name"), { target: { value: "payments" } });
    await userEvent.click(screen.getByTestId("choose-folder"));

    expect(screen.queryByTestId("pick-error")).not.toBeInTheDocument();
    expect(screen.getByTestId("template-empty")).toBeDisabled();
  });

  it("renders a pick failure inline under the folder field", async () => {
    const onchoosefolder = vi.fn(
      async (): Promise<PickOutcome> => ({ kind: "failed", message: "blocked by permissions policy" }),
    );
    render(NewProjectDialog, { props: { templates, onchoosefolder } });

    await userEvent.click(screen.getByTestId("choose-folder"));

    const error = screen.getByTestId("pick-error");
    expect(error).toHaveTextContent("blocked by permissions policy");
    expect(screen.getByTestId("choose-folder")).toHaveAttribute("aria-invalid", "true");
  });

  it("clears the inline failure once a pick succeeds", async () => {
    const folder = { name: "workspace" } as FileSystemDirectoryHandle;
    let next: PickOutcome = { kind: "failed", message: "boom" };
    const onchoosefolder = vi.fn(async (): Promise<PickOutcome> => next);
    render(NewProjectDialog, { props: { templates, onchoosefolder } });

    await userEvent.click(screen.getByTestId("choose-folder"));
    expect(screen.getByTestId("pick-error")).toBeInTheDocument();

    next = { kind: "picked", handle: folder };
    await userEvent.click(screen.getByTestId("choose-folder"));
    expect(screen.queryByTestId("pick-error")).not.toBeInTheDocument();
  });

  it("closes via the close button", async () => {
    const onclose = vi.fn();
    render(NewProjectDialog, { props: { templates, onclose } });
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalled();
  });
});
