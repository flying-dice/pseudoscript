import adapter from "@sveltejs/adapter-cloudflare";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Cloudflare Pages adapter. The app is client-rendered (SSR off, see
    // src/routes/+layout.js), so this ships a static shell + the wasm + JS.
    adapter: adapter(),
    // Register the service worker ourselves, prod-only (see +page.svelte). In
    // dev it would claim the client and intercept Vite's module/HMR requests,
    // which only adds flake; the PWA is a production concern.
    serviceWorker: { register: false },
  },
};

export default config;
