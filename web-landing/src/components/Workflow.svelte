<section id="workflow" data-screen-label="Workflow">
  <div class="wrap">
    <div class="eyebrow-row reveal" style="margin-bottom:2.5rem"><span class="lbl accent">How it works</span><span class="kicker-rule" style="flex:1"></span></div>
    <div class="steps">

      <!-- STEP 1: Describe -->
      <div class="step reveal">
        <div class="step-copy">
          <div class="step-index">01 — Describe</div>
          <h2 class="statement">You write structure, <span class="dim">not implementation.</span></h2>
          <p class="lede">A few lines of PseudoScript declare a bounded context and its guarantees. The intent is the code; the mechanism is named, then you stop.</p>
        </div>
        <div class="step-visual ticked">
          <span class="tick tl"></span><span class="tick br"></span>
          <div class="terminal-head"><div class="dots"><i></i><i></i><i></i></div><span class="fname"><b>inventory</b>.pds</span></div>
          <div class="editor-pane" style="border-right:none">
            <div class="code-block"><div class="gutter" id="describe-gutter"></div><div class="code" id="describe-code"></div></div>
          </div>
        </div>
      </div>

      <!-- STEP 2: Refine -->
      <div class="step flip reveal">
        <div class="step-copy">
          <div class="step-index">02 — Refine</div>
          <h2 class="statement">Iterate until the model is <span class="hot">well-formed.</span></h2>
          <p class="lede">The Linter flags missing docs, crossed references, and unreachable nodes as you go. Resolve them and the problem list empties.</p>
        </div>
        <div class="step-visual ticked" id="refine-visual">
          <span class="tick tl"></span><span class="tick br"></span>
          <div class="panel-head"><span class="pl">Problems</span><span class="sp"></span><span class="count" id="prob-count">3 problems</span></div>
          <ul class="problems-list" id="problems">
            <li class="error"><div class="row"><span class="badge">error</span><span class="prob-file">orders</span><span class="prob-loc">14:3</span><span class="prob-msg">unresolved reference <code>payments::Charge</code></span></div></li>
            <li class="warning"><div class="row"><span class="badge">warn</span><span class="prob-file">pricing</span><span class="prob-loc">7:3</span><span class="prob-msg">public node has no <code>///</code> summary</span></div></li>
            <li class="info"><div class="row"><span class="badge">info</span><span class="prob-file">shared</span><span class="prob-loc">1:1</span><span class="prob-msg">module doc <code>//!</code> recommended</span></div></li>
          </ul>
          <div class="well-formed" id="well-formed"><span class="ok-dot"></span> No problems — the model is well-formed.</div>
        </div>
      </div>

      <!-- STEP 3: Generate -->
      <div class="step reveal">
        <div class="step-copy">
          <div class="step-index">03 — Generate</div>
          <h2 class="statement">Artefacts your <span class="hot">agents</span> can build against.</h2>
          <p class="lede">Every diagram is projected from the spec — C4 context, container, component, and the sequence diagrams that trace a saga end to end. Machine-readable by construction.</p>
        </div>
        <div class="step-visual ticked">
          <span class="tick tl"></span><span class="tick br"></span>
          <div class="terminal-head"><div class="dots"><i></i><i></i><i></i></div><span class="fname">checkout · <b>sequence</b></span></div>
          <div class="seq" id="seq-visual">
            <svg viewBox="0 0 460 300" id="seq-svg" role="img" aria-label="Checkout saga sequence diagram"></svg>
          </div>
        </div>
      </div>

    </div>
  </div>
</section>
