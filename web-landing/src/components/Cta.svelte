<script lang="ts">
  import { Monitor, Terminal, Blocks, ArrowRight } from '@lucide/svelte';
  import InstallCommand from './InstallCommand.svelte';
</script>

<section id="start" class="cta-band" data-screen-label="Get started">
  <div class="wrap">
    <div class="cta-head">
      <div class="eyebrow-row reveal" style="justify-content:center"><span class="lbl accent">Get started</span></div>
      <h2 class="statement reveal d1">Start describing.</h2>
      <p class="lede reveal d2" style="margin-left:auto;margin-right:auto">Same language, wherever you write it. Pick the surface that matches how you work — the spec comes with you.</p>
    </div>
    <div class="start-grid">
      <a class="start-card reveal" href="http://ide.pdscript.dev/">
        <span class="ct tl"></span><span class="ct br"></span>
        <span class="sc-icon"><Monitor strokeWidth={1.75} /></span>
        <span class="sc-step">Fastest</span>
        <h3>Start with the IDE</h3>
        <p>A fully contained environment with the Analyzer, Formatter, Linter, and LSP — all in WASM. Nothing to install.</p>
        <span class="sc-tag"><b>ide.pdscript.dev</b></span>
        <span class="sc-go">Open the IDE <ArrowRight strokeWidth={1.75} /></span>
      </a>
      <a class="start-card reveal d1" href="https://github.com/flying-dice/pseudoscript/releases" target="_blank" rel="noreferrer">
        <span class="ct tl"></span><span class="ct br"></span>
        <span class="sc-icon"><Terminal strokeWidth={1.75} /></span>
        <span class="sc-step">Local</span>
        <h3>Download the CLI</h3>
        <p>Get started in your own editor. Analyze, format, lint, and build docs from the command line, wired into your toolchain.</p>
        <span class="sc-tag"><b>pds build</b> · <b>pds doc</b></span>
        <span class="sc-go">Install the CLI <ArrowRight strokeWidth={1.75} /></span>
      </a>
      <a class="start-card reveal d2" href="https://github.com/flying-dice/pseudoscript" target="_blank" rel="noreferrer">
        <span class="ct tl"></span><span class="ct br"></span>
        <span class="sc-icon"><Blocks strokeWidth={1.75} /></span>
        <span class="sc-step">In-editor</span>
        <h3>Install the extension</h3>
        <p>Inline diagnostics and live diagrams where you already work. JetBrains is supported today — more editors coming soon.</p>
        <span class="sc-tag"><b>JetBrains</b> · more soon</span>
        <span class="sc-go">Get the extension <ArrowRight strokeWidth={1.75} /></span>
      </a>
    </div>

    <InstallCommand />
  </div>
</section>
