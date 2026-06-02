import { describe, expect, it } from "vitest";

import { byteToChar, charToByte } from "./offsets.js";

// The compiler reports UTF-8 byte offsets; CodeMirror counts UTF-16 code units.
// These conversions back the semantic-highlight and fold decorations, so a
// rounding error mis-places every colour. ASCII coincides; multi-byte diverges.
describe("offsets", () => {
  it("is the identity for pure-ASCII source", () => {
    const src = "system S { run() {} }";
    for (let i = 0; i <= src.length; i++) {
      expect(byteToChar(src, i)).toBe(i);
      expect(charToByte(src, i)).toBe(i);
    }
  });

  it("accounts for a multi-byte character (é = 2 bytes)", () => {
    const src = "café = 1"; // 'é' is 2 UTF-8 bytes, 1 code unit
    // char index 4 (the space after café) sits at byte 5.
    expect(charToByte(src, 4)).toBe(5);
    expect(byteToChar(src, 5)).toBe(4);
  });

  it("round-trips char → byte → char across a multi-byte string", () => {
    const src = "α: number // δ"; // α and δ are 2 bytes each
    for (let i = 0; i <= src.length; i++) {
      expect(byteToChar(src, charToByte(src, i))).toBe(i);
    }
  });

  it("handles an astral character (😀 = 4 bytes, 2 code units)", () => {
    const src = "x😀y";
    expect(charToByte(src, 0)).toBe(0);
    expect(charToByte(src, 1)).toBe(1); // after 'x'
    expect(charToByte(src, 3)).toBe(5); // after the surrogate pair
    expect(byteToChar(src, 5)).toBe(3);
  });

  it("clamps out-of-range offsets to the bounds", () => {
    const src = "abc";
    expect(byteToChar(src, -4)).toBe(0);
    expect(byteToChar(src, 99)).toBe(3);
    expect(charToByte(src, -1)).toBe(0);
    expect(charToByte(src, 99)).toBe(3);
  });
});
