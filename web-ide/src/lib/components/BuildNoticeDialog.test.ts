import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import "@testing-library/jest-dom/vitest";

import BuildNoticeDialog from "./BuildNoticeDialog.svelte";

describe("BuildNoticeDialog", () => {
  it("renders the notice without any preview action", () => {
    render(BuildNoticeDialog, { props: {} });
    expect(screen.getByTestId("build-notice")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /preview/i })).not.toBeInTheDocument();
  });

  it("offers the open-folder action only when onopenfolder is set", async () => {
    const onopenfolder = vi.fn();
    const { unmount } = render(BuildNoticeDialog, { props: {} });
    expect(screen.queryByRole("button", { name: "Open a folder…" })).not.toBeInTheDocument();
    unmount();
    render(BuildNoticeDialog, { props: { onopenfolder } });
    await userEvent.click(screen.getByRole("button", { name: "Open a folder…" }));
    expect(onopenfolder).toHaveBeenCalled();
  });

  it("cancels", async () => {
    const oncancel = vi.fn();
    render(BuildNoticeDialog, { props: { oncancel } });
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(oncancel).toHaveBeenCalled();
  });
});
