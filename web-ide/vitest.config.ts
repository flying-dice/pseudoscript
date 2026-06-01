import { fileURLToPath } from "node:url";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Component/unit tests run under jsdom with the Svelte plugin (not the SvelteKit
// plugin, which expects SSR). The `browser` resolve condition makes Svelte 5's
// client runtime load in jsdom. The `$lib` alias mirrors SvelteKit's, so deep
// shadcn-svelte imports (`$lib/components/ui/…`) resolve in tests as in the app.
// Playwright e2e lives separately under e2e/.
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ["browser"],
    alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest-setup.ts"],
    include: ["src/**/*.{test,spec}.{js,ts}"],
  },
});
