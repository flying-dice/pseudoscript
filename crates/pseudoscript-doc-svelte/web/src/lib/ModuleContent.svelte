<script>
  import NodeSection from "./NodeSection.svelte";
  let { page } = $props();
</script>

<header class="page-head">
  <div class="eyebrow">Module</div>
  <h1 class="mono">{page.name}</h1>
</header>

{#each page.sections as section}
  <NodeSection {section} />
{/each}
