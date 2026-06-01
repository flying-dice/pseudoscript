// Adds jest-dom matchers (toBeInTheDocument, toHaveAttribute, …) and unmounts
// Svelte components rendered by @testing-library between tests.
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";

afterEach(() => cleanup());
