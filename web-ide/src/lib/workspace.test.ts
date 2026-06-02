import { describe, expect, it } from "vitest";

import { isBinaryPath } from "./workspace.js";

describe("isBinaryPath", () => {
  it("flags known binary extensions, case-insensitively", () => {
    expect(isBinaryPath("logo.png")).toBe(true);
    expect(isBinaryPath("img/Photo.JPG")).toBe(true);
    expect(isBinaryPath("fonts/Inter.woff2")).toBe(true);
    expect(isBinaryPath("a/b/diagram.pdf")).toBe(true);
  });

  it("treats text and code files as non-binary", () => {
    expect(isBinaryPath("README.md")).toBe(false);
    expect(isBinaryPath("data/config.json")).toBe(false);
    expect(isBinaryPath("icon.svg")).toBe(false); // editable XML, deliberately text
    expect(isBinaryPath("main.rs")).toBe(false);
  });

  it("returns false for an extensionless path", () => {
    expect(isBinaryPath("Makefile")).toBe(false);
    expect(isBinaryPath("dir/LICENSE")).toBe(false);
  });
});
