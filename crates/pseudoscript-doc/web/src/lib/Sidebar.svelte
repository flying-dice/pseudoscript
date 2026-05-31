<script>
  import TreeNode from "./TreeNode.svelte";
  let { site, docGroups = [], sidebar } = $props();
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
  {#if docGroups.length}
    <nav class="docs-nav" aria-label="Documentation">
      {#each docGroups as group}
        <div class="docs-group">
          <div class="docs-group-title">{group.title}</div>
          <ul class="docs-items">
            {#each group.items as item}
              <li><a class="docs-link" href={item.href}>{item.title}</a></li>
            {/each}
          </ul>
        </div>
      {/each}
    </nav>
  {/if}
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
