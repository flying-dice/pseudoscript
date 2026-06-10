<script>
  // Inline finding badges on a node section, each linking to the health page.
  let { diagnostics = [], prefix = "" } = $props();
</script>

{#if diagnostics.length}
  <div class="badges">
    {#each diagnostics as d}
      <a class="badge badge-{d.severity}" href="{prefix}health.html" title={d.message}>
        &#9888; {d.code || d.severity} &middot; L{d.line}
      </a>
    {/each}
  </div>
{/if}
