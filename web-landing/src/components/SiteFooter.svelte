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
        <p class="blurb">Architecture as code. Describe the system; generate the artefacts your agents read and build against.</p>
      </div>
      <div class="foot-cols">
        <div class="foot-col">
          <h4>Product</h4>
          <a href="http://ide.pdscript.dev/" target="_blank" rel="noreferrer">Web IDE</a>
          <a href="https://github.com/flying-dice/pseudoscript/releases" target="_blank" rel="noreferrer">CLI</a>
          <a href="https://github.com/flying-dice/pseudoscript" target="_blank" rel="noreferrer">JetBrains plugin</a>
          <a href="#packages">Packages</a>
        </div>
        <div class="foot-col">
          <h4>Language</h4>
          <a href="#converge">Overview</a>
          <a href="#workflow">Workflow</a>
          <a href="https://github.com/flying-dice/pseudoscript/blob/main/LANG.md" target="_blank" rel="noreferrer">LANG.md</a>
          <a href="https://github.com/flying-dice/pseudoscript/blob/main/PATTERNS.md" target="_blank" rel="noreferrer">Patterns</a>
        </div>
        <div class="foot-col">
          <h4>Resources</h4>
          <a href="https://github.com/flying-dice/pseudoscript#readme" target="_blank" rel="noreferrer">Docs</a>
          <a href="https://github.com/flying-dice/pseudoscript/tree/main/web-ide/src/lib/samples/acme-tickets" target="_blank" rel="noreferrer">Worked example</a>
          <a href="https://github.com/flying-dice/pseudoscript/tree/main/decisions" target="_blank" rel="noreferrer">Decision records</a>
          <a href="https://github.com/flying-dice/pseudoscript/releases" target="_blank" rel="noreferrer">Changelog</a>
        </div>
      </div>
    </div>
    <div class="foot-bottom">
      <span class="copy">PseudoScript — the spec is the source of truth.</span>
      <span class="copy"><code>pds 1.4.0</code> · drafting terminal</span>
    </div>
  </div>
</footer>
