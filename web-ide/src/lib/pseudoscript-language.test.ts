import { describe, expect, it, vi } from "vitest";

// Avoid pulling the wasm module: the language only needs semanticTokens to exist.
vi.mock("./pds.js", () => ({ semanticTokens: () => ({ data: [] }) }));

import { LanguageSupport } from "@codemirror/language";

import { pseudoscript, pseudoscriptCompletion, pseudoscriptLinter } from "./pseudoscript-language.js";

// Smoke only: highlighting, completion narrowing and inline diagnostics need a
// mounted EditorView + wasm and are e2e-covered (ide.spec.ts). This guards the
// factories assemble and the byte→char/severity wiring loads.
describe("pseudoscript-language factories", () => {
  it("pseudoscript() returns LanguageSupport", () => {
    expect(pseudoscript()).toBeInstanceOf(LanguageSupport);
  });

  it("pseudoscriptCompletion returns a defined extension", () => {
    expect(pseudoscriptCompletion(() => [])).toBeDefined();
  });

  it("pseudoscriptLinter returns a defined extension", () => {
    expect(pseudoscriptLinter(() => [])).toBeDefined();
  });
});
