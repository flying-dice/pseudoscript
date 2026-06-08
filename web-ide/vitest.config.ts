import { fileURLToPath } from "node:url";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Component/unit tests run under jsdom with the Svelte plugin (not the SvelteKit
// plugin, which expects SSR). The `browser` resolve condition makes Svelte 5's
// client runtime load in jsdom. The `$lib` alias mirrors SvelteKit's, so deep
// shadcn-svelte imports (`$lib/components/ui/…`) resolve in tests as in the app.
// Playwright e2e lives separately under e2e/.
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ["browser"],
    alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest-setup.ts"],
    include: ["src/**/*.{test,spec}.{js,ts}"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      include: ["src/lib/**/*.{ts,svelte}"],
      // Excluded: vendored wasm + shadcn UI primitives, type-only modules, stories
      // and tests, and the render paths that only run under a real browser — WebGL
      // (Three.js), CodeMirror (Editor + live decorations), SvelteFlow canvas and
      // its SVG node/edge renderers, and the always-mounted shell chrome. Those are
      // covered by Playwright e2e, not vitest — see e2e/.
      exclude: [
        "src/lib/pds-ide-wasm/**",
        "src/lib/components/ui/**",
        "**/*.stories.{js,ts,svelte}",
        "**/*.{test,spec}.{js,ts}",
        "src/lib/core/diagram-scene.ts",
        "src/lib/core/types.ts",
        // The wasm facade: thin typed wrappers over the IdeSession. No DOM-less
        // surface — every e2e test drives it as a real client.
        "src/lib/pds.ts",
        // CodeMirror language/decoration glue — behaviour is e2e-covered
        // (ide.spec.ts: highlighting, completion, diagnostics, folding).
        "src/lib/pseudoscript-language.ts",
        "src/lib/markdown-live.ts",
        // WebGL / CodeMirror — no DOM-less surface to assert.
        "src/lib/components/ForceGraph.svelte",
        "src/lib/components/FitView.svelte",
        "src/lib/components/Editor.svelte",
        // SvelteFlow canvas + its SVG node/edge renderers (e2e: canvas/*.spec.ts).
        "src/lib/components/DiagramPane.svelte",
        "src/lib/components/C4Flow.svelte",
        "src/lib/components/C4Node.svelte",
        "src/lib/components/BoundaryNode.svelte",
        "src/lib/components/PolylineEdge.svelte",
        "src/lib/components/CanvasMenu.svelte",
        "src/lib/components/DiagramExport.svelte",
        "src/lib/components/LayoutControl.svelte",
        "src/lib/components/FlowTimeline.svelte",
        "src/lib/components/SequenceLifeline.svelte",
        "src/lib/components/SequenceFragment.svelte",
        "src/lib/components/SequenceMessages.svelte",
        "src/lib/components/DataModel.svelte",
        "src/lib/components/DataEntities.svelte",
        "src/lib/components/FeatureFlow.svelte",
        "src/lib/components/FeatureSteps.svelte",
        // Always-mounted shell chrome (e2e: panel-resize/tabs/ide specs).
        "src/lib/components/shell/TopBar.svelte",
        "src/lib/components/shell/MenuBar.svelte",
        "src/lib/components/shell/ActivityBar.svelte",
        "src/lib/components/shell/BottomDock.svelte",
        "src/lib/components/shell/CommandPalette.svelte",
        "src/lib/components/shell/PerfMeter.svelte",
        "src/lib/components/shell/RightRail.svelte",
        "src/lib/components/shell/Splitter.svelte",
      ],
      thresholds: { lines: 90, functions: 90, branches: 80 },
    },
  },
});
