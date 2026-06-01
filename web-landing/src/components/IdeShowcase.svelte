<script>
  import { FileCode, User, Box, Container, Database, Cpu, AlignLeft, ScanLine, Zap } from '@lucide/svelte';
</script>

<section id="ide" data-screen-label="IDE showcase">
  <div class="wrap">
    <div class="eyebrow-row reveal"><span class="lbl accent">The Web IDE</span></div>
    <h2 class="statement reveal d1" style="max-width:18ch">The whole toolchain, <span class="dim">in a browser tab.</span></h2>
    <p class="lede wide reveal d2" style="margin-bottom:2.6rem">No backend. Nothing to install. The Analyzer, Formatter, Linter, and language server are compiled to WebAssembly and run entirely on your machine.</p>

    <div class="ide-frame reveal d1">
      <div class="ide-top">
        <span class="brand"><span class="dot"></span><span class="word">PseudoScript</span></span>
        <span class="tb-ghost"><span class="acc">◳</span> acme-tickets</span>
        <span class="sp"></span>
        <span class="tb-format">Format</span>
        <span class="tb-status"><span class="d"></span> well-formed</span>
      </div>
      <div class="ide-body">
        <div class="ide-tree">
          <div class="tree-kicker">Workspace</div>
          <div class="trow file active"><FileCode strokeWidth={1.75} /> context.pds</div>
          <div class="trow file"><FileCode strokeWidth={1.75} /> inventory.pds</div>
          <div class="trow file"><FileCode strokeWidth={1.75} /> orders.pds</div>
          <div class="trow file"><FileCode strokeWidth={1.75} /> shared.pds</div>
          <div class="tree-kicker" style="margin-top:.7rem">Symbols</div>
          <div class="trow sym person"><User strokeWidth={1.75} /> Attendee</div>
          <div class="trow sym system"><Box strokeWidth={1.75} /> AcmeTickets</div>
          <div class="trow sym container"><Container strokeWidth={1.75} /> inventory</div>
          <div class="trow sym data"><Database strokeWidth={1.75} /> Hold</div>
        </div>
        <div class="ide-editor">
          <div class="gutter" id="ide-gutter"></div>
          <div class="code" id="ide-code"></div>
        </div>
        <div class="ide-diagram">
          <div class="grid-dots"></div>
          <div class="c4-stage" id="ide-stage">
            <svg class="c4-svg" viewBox="0 0 100 100" preserveAspectRatio="none">
              <line class="edge" x1="18" y1="50" x2="52" y2="28" pathLength="1" vector-effect="non-scaling-stroke" style="stroke-dashoffset:0"></line>
              <line class="edge" x1="52" y1="28" x2="52" y2="76" pathLength="1" vector-effect="non-scaling-stroke" style="stroke-dashoffset:0"></line>
              <line class="edge" x1="52" y1="28" x2="84" y2="50" pathLength="1" vector-effect="non-scaling-stroke" style="stroke-dashoffset:0"></line>
            </svg>
            <div class="c4-node container in" style="left:18%;top:50%"><span class="c4-kind">container</span><span class="c4-label">orders</span></div>
            <div class="c4-node component in" style="left:52%;top:28%"><span class="c4-kind">component</span><span class="c4-label">Pool</span></div>
            <div class="c4-node data in" style="left:52%;top:76%"><span class="c4-kind">data</span><span class="c4-label">Hold</span></div>
            <div class="c4-node data in" style="left:84%;top:50%"><span class="c4-kind">data</span><span class="c4-label">TierId</span></div>
          </div>
        </div>
      </div>
      <div class="ide-status">
        <span class="seg"><span class="d"></span> well-formed</span>
        <span class="seg">12 modules</span>
        <span class="seg">0 errors</span>
        <span class="sp"></span>
        <span class="seg">pds 1.4.0 · wasm</span>
      </div>
    </div>

    <div class="ide-caps reveal d2">
      <span class="cap"><Cpu strokeWidth={1.75} /> <b>Analyzer</b> — type &amp; reference checking</span>
      <span class="cap"><AlignLeft strokeWidth={1.75} /> <b>Formatter</b> — canonical layout</span>
      <span class="cap"><ScanLine strokeWidth={1.75} /> <b>Linter</b> — model hygiene</span>
      <span class="cap"><Zap strokeWidth={1.75} /> <b>LSP</b> — hover, go-to, rename</span>
    </div>
  </div>
</section>
