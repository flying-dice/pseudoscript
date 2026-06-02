import tailwindcss from "@tailwindcss/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  // The wasm-bindgen glue fetches the `.wasm` via `new URL(..., import.meta.url)`;
  // keep it out of dep pre-bundling so Vite emits the asset and resolves the URL.
  optimizeDeps: { exclude: ["$lib/pds-wasm/pseudoscript_wasm.js"] },
});
