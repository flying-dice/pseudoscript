<script>
  // The breadcrumb trail. The trailing crumb is this page: plain text marked
  // aria-current. Renders nothing on the index (empty trail).
  let { crumbs = [] } = $props();
</script>

{#if crumbs.length}
  <nav class="crumbs" aria-label="Breadcrumbs">
    <ol>
      {#each crumbs as crumb, i}
        <li>
          {#if crumb.href}
            <a href={crumb.href}>{crumb.label}</a>
          {:else}
            <span aria-current={i === crumbs.length - 1 ? "page" : undefined}>{crumb.label}</span>
          {/if}
        </li>
      {/each}
    </ol>
  </nav>
{/if}
