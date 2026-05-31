<script>
  // A UML combined fragment (alt / loop): a bordered box with a notched operator
  // tab, its guard, and a dashed divider per section split (each carrying the
  // following section's guard). The box fills the node; divider y's are relative
  // to it. Transparent fill and no pointer events, so the arrows read through.
  let { data } = $props();
</script>

<div class="seq-frag" title="{data.kind} [{data.label}]">
  <span class="seq-frag-tab">{data.kind}</span>
  <span class="seq-frag-guard">[{data.label}]</span>
  {#each data.dividers as d (d.y)}
    <div class="seq-frag-divider" style="top:{d.y}px"></div>
    <span class="seq-frag-else" style="top:{d.y}px">[{d.guard || "else"}]</span>
  {/each}
</div>

<style>
  .seq-frag {
    width: 100%;
    height: 100%;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    background: color-mix(in srgb, var(--ink) 2%, transparent);
    pointer-events: none;
  }
  .seq-frag-tab {
    position: absolute;
    top: 0;
    left: 0;
    padding: 0.05rem 0.7rem 0.1rem 0.4rem;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    color: var(--ink);
    background: var(--line-strong);
    /* notched lower-right corner */
    clip-path: polygon(0 0, 100% 0, calc(100% - 8px) 100%, 0 100%);
  }
  .seq-frag-guard {
    position: absolute;
    top: 0.1rem;
    left: 3.4rem;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-faint);
  }
  /* the alt split: a dashed rule separating two paths */
  .seq-frag-divider {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px dashed var(--line-strong);
  }
  .seq-frag-else {
    position: absolute;
    left: 0.5rem;
    margin-top: 0.15rem;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    color: var(--ink-faint);
  }
</style>
