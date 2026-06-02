<script lang="ts">
  import { ArrowRight } from '@lucide/svelte';
</script>

<section class="hero" data-screen-label="Hero">
  <div class="wrap hero-grid">
    <div class="hero-copy">
      <div class="eyebrow-row reveal">
        <span class="lbl accent" style="white-space:nowrap">Model-driven development</span>
        <span class="lbl" style="white-space:nowrap">pseudo-programming</span>
      </div>
      <h1 class="statement reveal d1">Model the intent. <span class="dim">Implement the detail.</span></h1>
      <p class="lede reveal d2">PseudoScript powers <a href="https://en.wikipedia.org/wiki/Model-driven_engineering" target="_blank" rel="noreferrer">model-driven development</a> and <a href="https://en.wikipedia.org/wiki/Pseudocode" target="_blank" rel="noreferrer">pseudo-programming</a> for the agentic era. Model your system in plain pseudocode. Your team and your agents read one always-current map, not diagrams that rot.</p>
      <div class="hero-cta reveal d3">
        <a class="btn btn-primary btn-lg" href="http://ide.pdscript.dev/">Open the Web IDE <span class="btn-arrow ico"><ArrowRight strokeWidth={1.75} /></span></a>
      </div>
      <p class="hero-alt reveal d4">Runs in your browser. Or <a href="#install">install the CLI</a> or the <a href="https://plugins.jetbrains.com/plugin/32021-pseudoscript" target="_blank" rel="noreferrer">JetBrains plugin</a>.</p>
    </div>

    <!-- drafting terminal: live code -> diagram -->
    <div class="terminal reveal d2" id="hero-terminal">
      <div class="terminal-head">
        <div class="dots"><i></i><i></i><i></i></div>
        <span class="fname"><b>context</b>.pds</span>
        <span class="sp"></span>
        <span class="status" id="hero-status"><span class="sdot"></span><span class="stext">idle</span></span>
      </div>
      <div class="terminal-body">
        <div class="editor-pane">
          <div class="code-block">
            <div class="gutter" id="hero-gutter"></div>
            <div class="code" id="hero-code"></div>
          </div>
        </div>
        <div class="sheet-pane" aria-hidden="true">
          <div class="grid-dots"></div>
          <span class="lbl depth-label">System context</span>
          <div class="c4-stage" id="hero-stage">
            <svg class="c4-svg" viewBox="0 0 100 100" preserveAspectRatio="none" id="hero-edges">
              <line class="edge" x1="15" y1="22" x2="49" y2="51" pathLength="1" vector-effect="non-scaling-stroke" data-i="0"></line>
              <line class="edge" x1="15" y1="80" x2="49" y2="51" pathLength="1" vector-effect="non-scaling-stroke" data-i="1"></line>
              <line class="edge" x1="49" y1="51" x2="80" y2="26" pathLength="1" vector-effect="non-scaling-stroke" data-i="2"></line>
              <line class="edge" x1="49" y1="51" x2="77" y2="80" pathLength="1" vector-effect="non-scaling-stroke" data-i="3"></line>
            </svg>
            <div class="c4-node person" style="left:15%;top:22%" data-i="0"><span class="c4-kind">person</span><span class="c4-label">Attendee</span></div>
            <div class="c4-node person" style="left:15%;top:80%" data-i="1"><span class="c4-kind">person</span><span class="c4-label">Organizer</span></div>
            <div class="c4-node system" style="left:49%;top:51%" data-i="2"><span class="c4-kind">system</span><span class="c4-label">AcmeTickets</span></div>
            <div class="c4-node system" style="left:80%;top:26%" data-i="3"><span class="c4-kind">system</span><span class="c4-label">Payments</span></div>
            <div class="c4-node system" style="left:77%;top:80%" data-i="4"><span class="c4-kind">system</span><span class="c4-label">Notifications</span></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</section>
