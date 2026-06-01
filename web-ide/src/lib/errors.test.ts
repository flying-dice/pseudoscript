import { afterEach, describe, expect, it, vi } from "vitest";

import { CODES, clearErrorLog, errorLog, onError, reportError } from "./errors.js";

afterEach(() => {
  clearErrorLog();
  vi.restoreAllMocks();
});

describe("error registry", () => {
  it("assigns every code a unique stable identifier", () => {
    const codes = Object.values(CODES).map((d) => d.code);
    expect(new Set(codes).size).toBe(codes.length);
    for (const code of codes) expect(code).toMatch(/^PDS-[A-Z]+-\d{3}$/);
  });

  it("logs errors via console.error and warnings via console.warn", () => {
    const err = vi.spyOn(console, "error").mockImplementation(() => {});
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    reportError("GOTO_UNRESOLVED", "M::Ghost"); // severity: error
    reportError("GOTO_MEMBER_FALLBACK", "M::Conv::id → M::Conv"); // severity: warn

    expect(err).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(err.mock.calls[0][0]).toContain(CODES.GOTO_UNRESOLVED.code);
    expect(err.mock.calls[0][0]).toContain("M::Ghost");
  });

  it("retains reports in the ring buffer and notifies listeners", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const seen: string[] = [];
    const off = onError((r) => seen.push(r.code));

    const report = reportError("WASM_CALL_FAILED", "definition: boom");

    expect(report.code).toBe(CODES.WASM_CALL_FAILED.code);
    expect(report.detail).toBe("definition: boom");
    expect(errorLog().at(-1)?.code).toBe(CODES.WASM_CALL_FAILED.code);
    expect(seen).toEqual([CODES.WASM_CALL_FAILED.code]);

    off();
    reportError("WASM_CALL_FAILED", "again");
    expect(seen).toHaveLength(1); // unsubscribed
  });
});
