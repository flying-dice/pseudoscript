// Client entry: hydrate the SSR markup with the identical props the server
// used (embedded as window.__DATA__), so server and client agree.
import { hydrate } from "svelte";
import Page from "./lib/Page.svelte";

const target = document.getElementById("app");
if (target && window.__DATA__) {
  hydrate(Page, { target, props: window.__DATA__ });
}
