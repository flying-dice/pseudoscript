<script>
  // The 3D universe page shell: a host element the island mounts into and an
  // SSR fallback (systems + flows as text) the island hides when it boots.
  let { page } = $props();

  const systems = $derived(page.nodes.filter((n) => n.level === "system"));
  const hrefOf = (id) => {
    const entry = page.hrefs.find((h) => h.id === id);
    return entry ? entry.href : null;
  };
</script>

<header class="page-head">
  <div class="eyebrow">Model</div>
  <h1>Universe</h1>
  <p class="lead">
    The whole model as a 3D scene &mdash; drag to orbit, scroll to zoom, click a
    node to open its docs.
  </p>
</header>

<div class="universe-host" data-universe></div>

<div class="universe-fallback" data-universe-fallback>
  {#if systems.length}
    <h2>Systems</h2>
    <ul class="universe-list">
      {#each systems as node}
        <li>
          {#if hrefOf(node.id)}
            <a class="fqn" href={hrefOf(node.id)}>{node.id}</a>
          {:else}
            <span class="fqn">{node.id}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  {#if page.flows.length}
    <h2>Flows</h2>
    <ul class="universe-list">
      {#each page.flows as flow}
        <li>
          <span class="flow-name">{flow.name}</span>
          <span class="flow-meta">{flow.hops.length} hop{flow.hops.length === 1 ? "" : "s"}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>
