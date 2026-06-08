// Adds jest-dom matchers (toBeInTheDocument, toHaveAttribute, …) and unmounts
// Svelte components rendered by @testing-library between tests.
import "@testing-library/jest-dom/vitest";
import { afterAll, afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";

afterEach(() => cleanup());

// bits-ui's Dialog scroll-lock schedules a deferred (~24ms) body-style reset when
// a dialog unmounts (bits-ui#1639). The final test's unmount leaves one timer
// pending; if it fires after the file's jsdom env is torn down it throws a benign
// `document`/`Element is not defined`, which vitest reports as an unhandled error.
// Flush that trailing timer once per file, while the env is still alive.
afterAll(async () => {
  await new Promise((resolve) => setTimeout(resolve, 30));
});
