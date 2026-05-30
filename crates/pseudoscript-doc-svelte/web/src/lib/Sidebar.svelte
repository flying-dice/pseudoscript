<script>
  import TreeNode from "./TreeNode.svelte";
  let { site, sidebar } = $props();
</script>

<aside class="sidebar">
  <div class="brand">
    <a class="brand-link" href="{site.prefix}index.html">
      {#if site.logoFilename}<img src="{site.prefix}{site.logoFilename}" alt="" />{/if}
      <span class="title">{site.name}<small>PseudoScript</small></span>
    </a>
  </div>
  <div class="search">
    <input
      type="search"
      placeholder="Filter nodes…"
      aria-label="Filter nodes"
      autocomplete="off"
      spellcheck="false"
    />
  </div>
  <ul class="tree">
    {#each sidebar as module}
      <li class="module" data-search={module.label}>
        <div class="row">
          <span class="toggle">&#9662;</span>
          <a class="label" href={module.href}>{module.label}</a>
        </div>
        <ul class="children">
          {#each module.nodes as node}<TreeNode {node} />{/each}
        </ul>
      </li>
    {/each}
  </ul>
</aside>
