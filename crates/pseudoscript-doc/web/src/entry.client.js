// Client entry: pure progressive enhancement — no Svelte, no hydration. The
// SSR markup is final; behaviors.js wires the interactive bits by DOM.
import { initBehaviors } from "./lib/behaviors.js";

if (document.readyState === "loading") {
  addEventListener("DOMContentLoaded", () => initBehaviors(document));
} else {
  initBehaviors(document);
}
