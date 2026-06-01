/// <reference types="@sveltejs/kit" />
/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="webworker" />

// The IDE runs entirely client-side (the compiler is wasm, there is no backend),
// so it works offline once cached — which is also what makes it an installable
// PWA. This worker precaches the built app shell + static assets on install,
// serves them cache-first, and falls back to the cache when the network is gone.

import { build, files, version } from "$service-worker";

const sw = self as unknown as ServiceWorkerGlobalScope;

const CACHE = `pds-cache-${version}`;
// Hashed build output + static assets (icons, wasm, manifest, …) — safe to
// cache-first since their URLs change when their contents do.
const PRECACHE = [...build, ...files];

sw.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(PRECACHE))
      .then(() => sw.skipWaiting()),
  );
});

sw.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
      .then(() => sw.clients.claim()),
  );
});

sw.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== location.origin) return; // never intercept cross-origin

  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE);

      // A precached asset: cache-first (it's content-addressed / static).
      if (PRECACHE.includes(url.pathname)) {
        const cached = await cache.match(url.pathname);
        if (cached) return cached;
      }

      // Otherwise network-first, falling back to the cache when offline.
      try {
        const response = await fetch(request);
        if (response.ok) cache.put(request, response.clone());
        return response;
      } catch {
        const cached = await cache.match(request);
        if (cached) return cached;
        throw new Error("offline and not cached");
      }
    })(),
  );
});
