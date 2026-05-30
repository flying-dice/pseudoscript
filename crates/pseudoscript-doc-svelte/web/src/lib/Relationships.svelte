<script>
  let { groups } = $props();
</script>

{#if groups.length}
  <div class="rel">
    {#each groups as group}
      <div class="rel-group">
        <h3>{group.title}</h3>
        <ul class="rel-list">
          {#each group.items as item}
            <li>
              <span class="edge-kind">{item.edgeKind}</span>
              {#if item.arrow}<span class="arrow">&rarr;</span> {/if}
              {#if item.href}
                <a class="fqn" href={item.href}>{item.fqn}</a>
              {:else}
                <span class="fqn">{item.fqn}</span>
              {/if}
              {#if item.label}<span class="edge-label">&middot; {item.label}</span>{/if}
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </div>
{/if}
