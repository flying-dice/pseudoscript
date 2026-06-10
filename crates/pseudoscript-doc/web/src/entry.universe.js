// The universe page's 3D island entry. Reads window.__DATA__ (embedded only on
// universe.html), probes WebGL, and mounts the three.js island into the
// [data-universe] host — hiding the SSR fallback on success, leaving it (plus a
// short note) when WebGL is unavailable. Remounts on `pds-themechange` so the
// scene re-reads the theme's CSS custom properties. IIFE-safe: no top-level await.
import { mountUniverse } from "./universe/island.js";

(function () {
  const data = window.__DATA__;
  const host = document.querySelector("[data-universe]");
  const page = data && data.page;
  if (!host || !page || !Array.isArray(page.nodes)) return;

  const probe = document.createElement("canvas");
  const gl = probe.getContext("webgl2") || probe.getContext("webgl");
  if (!gl) {
    // No WebGL: the SSR fallback stays visible; the host shows a short note.
    const note = document.createElement("p");
    note.className = "uv-nogl";
    note.textContent = "3D view requires WebGL.";
    host.appendChild(note);
    return;
  }

  const fallback = document.querySelector("[data-universe-fallback]");
  if (fallback) fallback.hidden = true;

  let dispose = mountUniverse(host, page);
  // The theme toggle dispatches `pds-themechange`; remount so the scene's colours
  // (backdrop, fog, lines, labels) re-read the now-active CSS variables.
  window.addEventListener("pds-themechange", () => {
    if (dispose) dispose();
    dispose = mountUniverse(host, page);
  });
})();
