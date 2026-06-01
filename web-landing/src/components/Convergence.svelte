<script lang="ts">
  import Mark from './Mark.svelte';
</script>

<section id="converge" data-screen-label="Convergence">
  <div class="wrap">
    <div class="converge-head">
      <div class="eyebrow-row reveal" style="justify-content:center"><span class="lbl accent">One toolchain</span></div>
      <h2 class="statement reveal d1">Four disciplines, <span class="hot">one practice.</span></h2>
      <p class="lede reveal d2">Spec-driven development, agentic coding, refinement, and design were separate steps. PseudoScript collapses them into a single source you write — and everything downstream regenerates.</p>
    </div>

    <div class="flow">
      <div class="flow-col">
        <div class="disc k-spec reveal"><div class="num">01 · INPUT</div><h3>Spec-driven development</h3><p>The model is the source. You declare structure — system, container, component, person — and the Analyzer holds you to it.</p></div>
        <div class="disc k-vibe reveal d1"><div class="num">02 · INPUT</div><h3>Agentic coding</h3><p>Describe intent in high-level pseudocode. No boilerplate, no ceremony — just what the system is and does.</p></div>
      </div>

      <div class="flow-core">
        <div class="core-mark" id="core-mark">
          <span class="pulse-ring"></span>
          <Mark strokeWidth={1.2} />
          <span class="core-word">Pseudo programming</span>
        </div>
      </div>

      <div class="flow-col">
        <div class="disc k-refine reveal d2"><div class="num">03 · LOOP</div><h3>Refinement</h3><p>Live diagnostics shape the model as you type. You iterate until there are no problems — the model is well-formed.</p></div>
        <div class="disc k-design reveal d3"><div class="num">04 · OUTPUT</div><h3>Design</h3><p>C4 models and sequence diagrams render straight from the spec. The picture is never stale, because it is generated.</p></div>
      </div>
    </div>
  </div>
</section>
