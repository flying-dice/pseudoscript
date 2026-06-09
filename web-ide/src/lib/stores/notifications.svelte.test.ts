import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { notifications } from "./notifications.svelte.js";

beforeEach(() => {
  notifications.notes = [];
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe("NotificationsStore", () => {
  it("pushes a toast with an ascending id", () => {
    notifications.notify("info", "First");
    notifications.notify("error", "Second");
    expect(notifications.notes.map((n) => n.title)).toEqual(["First", "Second"]);
    expect(notifications.notes[1].id).toBeGreaterThan(notifications.notes[0].id);
  });

  it("auto-dismisses a title-only toast after 3s", () => {
    notifications.notify("success", "Saved");
    vi.advanceTimersByTime(2999);
    expect(notifications.notes).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(notifications.notes).toHaveLength(0);
  });

  it("keeps a body toast for 6s", () => {
    notifications.notify("info", "Built", "wrote target/doc");
    vi.advanceTimersByTime(5999);
    expect(notifications.notes).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(notifications.notes).toHaveLength(0);
  });

  it("keeps an error toast for 9s", () => {
    notifications.notify("error", "Boom", "stack");
    vi.advanceTimersByTime(8999);
    expect(notifications.notes).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(notifications.notes).toHaveLength(0);
  });

  it("dismiss removes one toast by id, leaving the rest", () => {
    notifications.notify("info", "A");
    notifications.notify("info", "B");
    const [a] = notifications.notes;
    notifications.dismiss(a.id);
    expect(notifications.notes.map((n) => n.title)).toEqual(["B"]);
    notifications.dismiss(9999); // unknown id → no-op
    expect(notifications.notes).toHaveLength(1);
  });
});
