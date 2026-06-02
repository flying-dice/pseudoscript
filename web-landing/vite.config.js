import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  server: {
    // allow the Tailscale-proxied host (tailscale serve sets Host to the .ts.net name)
    allowedHosts: ['.ts.net'],
  },
});
