<script lang="ts"></script>

<section id="workflow" data-screen-label="Workflow">
  <div class="wrap">
    <div class="eyebrow-row reveal" style="margin-bottom:2.5rem"><span class="lbl accent">How it works</span><span class="kicker-rule" style="flex:1"></span></div>
    <div class="steps">

      <!-- STEP 1: Describe -->
      <div class="step reveal">
        <div class="step-copy">
          <h2 class="statement">Model intent, <span class="dim">not implementation.</span></h2>
          <p class="lede">Capture how the system works in a few lines of pseudocode. There are no diagrams to maintain and no docs to keep in sync.</p>
        </div>
        <div class="step-visual">
          <div class="terminal-head"><div class="dots"><i></i><i></i><i></i></div><span class="fname"><b>inventory</b>.pds</span></div>
          <div class="editor-pane" style="border-right:none">
            <div class="code-block"><div class="gutter" id="describe-gutter"></div><div class="code" id="describe-code"></div></div>
          </div>
        </div>
      </div>

      <!-- STEP 2: Refine -->
      <div class="step flip reveal">
        <div class="step-copy">
          <h2 class="statement">Iterate until the model is <span class="hot">well-formed.</span></h2>
          <p class="lede">The Analyzer knows your whole model. The moment something isn't valid, it tells you and helps you fix it.</p>
        </div>
        <div class="step-visual" id="refine-visual">
          <div class="panel-head"><span class="pl">Problems</span><span class="sp"></span><span class="count" id="prob-count">3 problems</span></div>
          <ul class="problems-list" id="problems">
            <li class="error"><div class="row"><span class="badge">error</span><span class="prob-file">orders</span><span class="prob-loc">8:17</span><span class="prob-msg">argument 1: expected <code>Money</code>, found <code>string</code></span></div></li>
            <li class="error"><div class="row"><span class="badge">error</span><span class="prob-file">orders</span><span class="prob-loc">9:9</span><span class="prob-msg">callable <code>add</code> expects 2 argument(s), got 3</span></div></li>
            <li class="error"><div class="row"><span class="badge">error</span><span class="prob-file">orders</span><span class="prob-loc">10:9</span><span class="prob-msg"><code>self.refund</code> does not name a callable of <code>C</code></span></div></li>
          </ul>
          <div class="well-formed" id="well-formed"><span class="ok-dot"></span> No problems. The model is well-formed.</div>
        </div>
      </div>

      <!-- STEP 3: Generate -->
      <div class="step reveal">
        <div class="step-copy">
          <h2 class="statement">Artefacts projected <span class="hot">straight from the model.</span></h2>
          <p class="lede">C4 context, containers, sequence diagrams that trace a saga end to end. All generated, all current.</p>
        </div>
        <div class="step-visual">
          <div class="terminal-head"><div class="dots"><i></i><i></i><i></i></div><span class="fname">checkout · <b>sequence</b></span><span class="sp"></span><span class="provenance lbl"><i class="pv-dot"></i> from the model</span></div>
          <div class="seq" id="seq-visual">
            <svg viewBox="0 0 460 300" id="seq-svg" role="img" aria-label="Checkout saga sequence diagram"></svg>
          </div>
        </div>
      </div>

    </div>
  </div>
</section>