<script>
  // The architecture-health report: a summary line and a semantic findings
  // table. Counts arrive precomputed; no graph logic here.
  let { page } = $props();
</script>

<header class="page-head">
  <div class="eyebrow">Report</div>
  <h1>Architecture health</h1>
</header>

{#if page.entries.length}
  <p class="health-summary">
    {page.errorCount} error{page.errorCount === 1 ? "" : "s"} &middot;
    {page.warningCount} warning{page.warningCount === 1 ? "" : "s"}
  </p>
  <table class="health-table">
    <thead>
      <tr>
        <th>Severity</th>
        <th>Code</th>
        <th>Location</th>
        <th>Message</th>
        <th>Node</th>
      </tr>
    </thead>
    <tbody>
      {#each page.entries as entry}
        <tr>
          <td data-label="Severity"><span class="badge badge-{entry.severity}">{entry.severity}</span></td>
          <td data-label="Code">
            {#if entry.code && entry.codeUrl}
              <a href={entry.codeUrl}>{entry.code}</a>
            {:else if entry.code}
              {entry.code}
            {/if}
          </td>
          <td data-label="Location"><code>{entry.module}:{entry.line}:{entry.column}</code></td>
          <td data-label="Message">{entry.message}</td>
          <td data-label="Node">
            {#if entry.nodeFqn}
              <a class="fqn" href={entry.href}>{entry.nodeFqn}</a>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{:else}
  <p class="health-summary">No findings.</p>
{/if}
