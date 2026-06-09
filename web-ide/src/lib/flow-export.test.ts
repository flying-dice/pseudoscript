import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { toPng, toSvg, getViewportForBounds } = vi.hoisted(() => ({
  toPng: vi.fn().mockResolvedValue("data:image/png;base64,AA"),
  toSvg: vi.fn().mockResolvedValue("data:image/svg+xml,BB"),
  getViewportForBounds: vi.fn(() => ({ x: 1, y: 2, zoom: 1 })),
}));

vi.mock("html-to-image", () => ({ toPng, toSvg }));
vi.mock("@xyflow/svelte", () => ({ getViewportForBounds }));

import { downloadDiagram } from "./flow-export.js";

function withViewport(): HTMLElement {
  const container = document.createElement("div");
  const vp = document.createElement("div");
  vp.className = "svelte-flow__viewport";
  container.appendChild(vp);
  return container;
}

const node = (id: string, x: number, y: number, w = 100, h = 60, parentId?: string) => ({
  id,
  position: { x, y },
  measured: { width: w, height: h },
  parentId,
});

let clickSpy: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  toPng.mockClear();
  toSvg.mockClear();
  getViewportForBounds.mockClear();
  clickSpy = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
});
afterEach(() => clickSpy.mockRestore());

describe("downloadDiagram guards", () => {
  it("rejects when there are no nodes", async () => {
    await expect(
      downloadDiagram(withViewport(), [], { format: "png", filename: "d", background: "#fff" }),
    ).rejects.toThrow(/nothing to export/);
  });

  it("rejects when the viewport element is missing", async () => {
    await expect(
      downloadDiagram(document.createElement("div"), [node("a", 0, 0)] as never, {
        format: "png",
        filename: "d",
        background: "#fff",
      }),
    ).rejects.toThrow(/diagram not ready/);
  });
});

describe("downloadDiagram happy paths", () => {
  it("renders PNG at pixelRatio 2 and downloads .png", async () => {
    await downloadDiagram(withViewport(), [node("a", 0, 0)] as never, {
      format: "png",
      filename: "diagram",
      background: "#111",
    });
    expect(toPng).toHaveBeenCalledTimes(1);
    expect(toPng.mock.calls[0][1]).toMatchObject({ pixelRatio: 2, backgroundColor: "#111" });
    expect(clickSpy).toHaveBeenCalledTimes(1);
  });

  it("renders SVG and downloads .svg", async () => {
    await downloadDiagram(withViewport(), [node("a", 0, 0)] as never, {
      format: "svg",
      filename: "diagram",
      background: "#111",
    });
    expect(toSvg).toHaveBeenCalledTimes(1);
    expect(toPng).not.toHaveBeenCalled();
  });
});

describe("bounds", () => {
  it("resolves a child's absolute position through its parent chain", async () => {
    await downloadDiagram(
      withViewport(),
      [node("parent", 1000, 1000), node("child", 50, 50, 100, 60, "parent")] as never,
      { format: "png", filename: "d", background: "#fff" },
    );
    const bounds = (getViewportForBounds.mock.calls[0] as unknown[])[0] as { x: number; y: number; width: number; height: number };
    // child sits at absolute (1050,1050); bounds span parent(1000,1000)..child(1150,1110).
    expect(bounds.x).toBe(1000);
    expect(bounds.width).toBe(1150 - 1000);
    expect(bounds.height).toBe(1110 - 1000);
  });

  it("caps the long edge at 4096 px preserving aspect ratio", async () => {
    await downloadDiagram(withViewport(), [node("a", 0, 0, 10000, 5000)] as never, {
      format: "png",
      filename: "d",
      background: "#fff",
    });
    const common = toPng.mock.calls[0][1] as { width: number; height: number };
    expect(common.width).toBe(4096);
    expect(common.height).toBe(Math.ceil(5000 * (4096 / 10000)));
  });
});
