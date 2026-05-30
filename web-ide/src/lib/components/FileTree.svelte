<script>
  let { workspaceName = "", files = [], openPath = null, onopen, errorPaths = new Set() } = $props();
</script>

<nav class="tree" aria-label="Workspace modules">
  <div class="head">
    <span class="kicker">Workspace</span>
    <span class="name" title={workspaceName}>{workspaceName}</span>
  </div>

  {#if files.length === 0}
    <div class="empty">No <code>.pds</code> modules here.</div>
  {:else}
    <ul>
      {#each files as file}
        <li>
          <button
            class="file"
            class:active={file.path === openPath}
            class:has-error={errorPaths.has(file.path)}
            onclick={() => onopen?.(file)}
            aria-label={file.path}
            aria-current={file.path === openPath ? "true" : undefined}
            title={file.path}
          >
            <span class="glyph" aria-hidden="true"></span>
            <span class="fqn">{file.fqn}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</nav>

<style>
  .tree {
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.9rem 0.95rem 0.7rem;
    border-bottom: 1px solid var(--line);
  }
  .kicker {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    font-weight: 600;
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .name {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--ink-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 0.8rem 0.95rem;
    font-size: 0.78rem;
    color: var(--ink-faint);
  }
  ul { list-style: none; margin: 0; padding: 0.4rem; }
  .file {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.34rem 0.5rem;
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.79rem;
  }
  .file .glyph {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 1.5px;
    background: var(--line-strong);
    transition: background 0.13s, transform 0.13s;
  }
  .file:hover { background: var(--surface-2); color: var(--ink); }
  .file:hover .glyph { background: var(--ink-faint); }
  .file.active { background: var(--accent-soft); color: var(--accent); }
  .file.active .glyph { background: var(--accent); transform: rotate(45deg); }
  .file.has-error .fqn::after {
    content: "";
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-left: 0.45rem;
    border-radius: 50%;
    background: var(--err);
    vertical-align: middle;
  }
  .fqn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
