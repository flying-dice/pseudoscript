<script lang="ts">
  import Mark from './Mark.svelte';
</script>

<footer data-screen-label="Footer">
  <div class="wrap">
    <div class="foot-grid">
      <div class="foot-brand">
        <a class="brand" href="#top">
          <Mark />
          <span class="word">PseudoScript</span>
        </a>
        <p class="blurb">Architecture as code. The model is the source. Describe the system once and generate the artefacts.</p>
      </div>
      <div class="foot-cols">
        <div class="foot-col">
          <h4>Product</h4>
          <a href="http://ide.pdscript.dev/" target="_blank" rel="noreferrer">Web IDE</a>
          <a href="https://github.com/flying-dice/pseudoscript/releases" target="_blank" rel="noreferrer">CLI</a>
          <a href="https://plugins.jetbrains.com/plugin/32021-pseudoscript" target="_blank" rel="noreferrer">JetBrains plugin</a>
        </div>
        <div class="foot-col">
          <h4>Language</h4>
          <a href="#workflow">How it works</a>
          <a href="#agents">With an agent</a>
          <a href="https://github.com/flying-dice/pseudoscript/blob/main/LANG.md" target="_blank" rel="noreferrer">LANG.md</a>
          <a href="https://github.com/flying-dice/pseudoscript/blob/main/PATTERNS.md" target="_blank" rel="noreferrer">Patterns</a>
        </div>
        <div class="foot-col">
          <h4>Resources</h4>
          <a href="https://en.wikipedia.org/wiki/Model-driven_engineering" target="_blank" rel="noreferrer">Model-driven engineering ↗</a>
          <a href="https://en.wikipedia.org/wiki/Pseudocode" target="_blank" rel="noreferrer">Pseudocode ↗</a>
          <a href="https://github.com/flying-dice/pseudoscript#readme" target="_blank" rel="noreferrer">Docs</a>
          <a href="https://github.com/flying-dice/pseudoscript/tree/main/web-ide/src/lib/samples/acme-tickets" target="_blank" rel="noreferrer">Worked example</a>
          <a href="https://github.com/flying-dice/pseudoscript/tree/main/decisions" target="_blank" rel="noreferrer">Decision records</a>
          <a href="https://github.com/flying-dice/pseudoscript/releases" target="_blank" rel="noreferrer">Changelog</a>
        </div>
      </div>
    </div>
    <div class="foot-bottom">
      <span class="copy">PseudoScript. The model is the source of truth.</span>
    </div>
  </div>
</footer>
