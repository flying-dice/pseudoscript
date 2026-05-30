// SSR entry, bundled to an IIFE exposing globalThis.SSR.renderPage.
// JSON in, JSON out — the simplest, most robust thing to pass across the
// Rust/QuickJS boundary.
import { render } from "svelte/server";
import Page from "./lib/Page.svelte";

export function renderPage(propsJson) {
  const props = JSON.parse(propsJson);
  const { head, body } = render(Page, { props });
  return JSON.stringify({ head, body });
}
