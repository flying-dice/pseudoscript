import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Component/unit tests run under jsdom with the Svelte plugin (not the SvelteKit
// plugin, which expects SSR). The `browser` resolve condition makes Svelte 5's
// client runtime load in jsdom. Playwright e2e lives separately under e2e/.
export default defineConfig({
  plugins: [svelte()],
  resolve: { conditions: ["browser"] },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest-setup.js"],
    include: ["src/**/*.{test,spec}.{js,ts}"],
  },
});
