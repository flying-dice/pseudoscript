import tailwindcss from "@tailwindcss/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  // Mirror the browser console to this terminal during dev, so the IDE's
  // diagnostic logs (`[pds-deps]`, `[pds-hover]`) are visible where the server
  // runs without copy/pasting from DevTools.
  server: { forwardConsole: { logLevels: ["info", "log", "warn", "error"] } },
  // The wasm-bindgen glue fetches the `.wasm` via `new URL(..., import.meta.url)`;
  // keep it out of dep pre-bundling so Vite emits the asset and resolves the URL.
  optimizeDeps: {
    exclude: ["$lib/pds-ide-wasm/pseudoscript_ide.js"],
  },
});
