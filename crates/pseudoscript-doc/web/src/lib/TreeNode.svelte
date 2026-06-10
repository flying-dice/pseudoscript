<script>
  // Recursive: a node renders its same-module children with itself.
  import Self from "./TreeNode.svelte";
  let { node } = $props();
</script>

<li data-search={node.fqn}>
  <div class="row">
    {#if node.children.length}
      <button class="toggle" type="button" aria-expanded="true" aria-label="Toggle {node.name}">&#9662;</button>
    {:else}
      <span class="toggle-spacer"></span>
    {/if}
    <a class="node-link" href={node.href}>
      <span class="kind-dot {node.kind}"></span>
      <span class="label">{node.name}</span>
    </a>
  </div>
  {#if node.children.length}
    <ul class="children">
      {#each node.children as child}<Self node={child} />{/each}
    </ul>
  {/if}
</li>
