<script>
  import NodeSection from "./NodeSection.svelte";
  let { page, prefix = "" } = $props();
</script>

<header class="page-head">
  <div class="eyebrow">Module</div>
  <h1 class="mono">{page.name}</h1>
</header>

{#each page.sections as section}
  <NodeSection {section} {prefix} />
{/each}
