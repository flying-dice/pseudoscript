// Client-only SPA: the editor and the wasm compiler both need the browser, so
// disable SSR. Prerender emits a static shell — ideal for Cloudflare Pages.
export const ssr = false;
export const prerender = true;
