<script>
  import { Box, Component, Container, Database, FileCode, FileText, SquareFunction, User } from "@lucide/svelte";

  // One icon per C4 level, so a node's place in the hierarchy reads at a glance.
  const ICONS = {
    person: User,
    system: Box,
    container: Container,
    component: Component,
    data: Database,
    callable: SquareFunction,
  };

  let {
    workspaceName = "",
    files = [],
    openPath = null,
    onopen,
    onpicknode,
    errorPaths = new Set(),
    // Authored doc groups from `[[doc.sidebar]]` (`{ title, items: [{ title,
    // path }] }`), listed above Files. Clicking a page opens its raw Markdown.
    docGroups = [],
    ondocopen,
    // Every declared node as { fqn, name, kind, triggered, parent, fileFqn },
    // nested by structural `parent` into the whole-model symbol tree — separate
    // from the file list, since a node's parent may live in another module.
    symbols = [],
    // The FQN of the currently selected node, highlighted in the tree.
    selectedFqn = null,
  } = $props();

  // Collapsed symbol subtrees, by node FQN. Default expanded.
  let collapsed = $state(new Set());
  function toggle(fqn) {
    const next = new Set(collapsed);
    next.has(fqn) ? next.delete(fqn) : next.add(fqn);
    collapsed = next;
  }

  // Nest the flat node list by structural `parent` (a node whose parent isn't in
  // the set — a top-level person/system/data — is a root).
  const tree = $derived.by(() => {
    const byFqn = new Map(symbols.map((n) => [n.fqn, n]));
    const children = new Map();
    const roots = [];
    for (const n of symbols) {
      if (n.parent && byFqn.has(n.parent)) {
        if (!children.has(n.parent)) children.set(n.parent, []);
        children.get(n.parent).push(n);
      } else {
        roots.push(n);
      }
    }
    const order = { person: 0, system: 1, container: 2, component: 3, data: 4, callable: 5 };
    const sort = (list) =>
      [...list].sort((a, b) => (order[a.kind] - order[b.kind]) || a.name.localeCompare(b.name));
    for (const [, list] of children) sort(list);
    return { roots: sort(roots), children };
  });
</script>

<nav class="tree" aria-label="Workspace">
  <div class="head">
    <span class="kicker">Workspace</span>
    <span class="name" title={workspaceName}>{workspaceName}</span>
  </div>

  {#if docGroups.length}
    <section class="group">
      <div class="group-head"><span class="kicker">Documentation</span></div>
      {#each docGroups as group}
        <div class="doc-group-title">{group.title}</div>
        <ul class="docs">
          {#each group.items as item}
            <li>
              <button
                class="doc"
                class:active={item.path === openPath}
                onclick={() => ondocopen?.(item)}
                aria-current={item.path === openPath ? "true" : undefined}
                title={item.path}
              >
                <FileText class="file-ico" size={15} strokeWidth={2} aria-hidden="true" />
                <span class="fqn">{item.title}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/each}
    </section>
  {/if}

  <section class="group">
    <div class="group-head"><span class="kicker">Files</span><span class="tally">{files.length}</span></div>
    {#if files.length === 0}
      <div class="empty">No <code>.pds</code> modules here.</div>
    {:else}
      <ul class="files">
        {#each files as file}
          <li>
            <button
              class="file"
              class:active={file.path === openPath}
              class:has-error={errorPaths.has(file.path)}
              onclick={() => onopen?.(file)}
              aria-current={file.path === openPath ? "true" : undefined}
              title={file.path}
            >
              <FileCode class="file-ico" size={15} strokeWidth={2} aria-hidden="true" />
              <span class="fqn">{file.fqn}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="group">
    <div class="group-head"><span class="kicker">Symbols</span><span class="tally">{symbols.length}</span></div>
    {#if tree.roots.length === 0}
      <div class="empty">No nodes declared yet.</div>
    {:else}
      <ul class="symbols">
        {#each tree.roots as node}{@render row(node, 0, tree.children)}{/each}
      </ul>
    {/if}
  </section>
</nav>

{#snippet row(node, depth, children)}
  {@const kids = children.get(node.fqn) ?? []}
  {@const Icon = ICONS[node.kind] ?? Box}
  {@const open = !collapsed.has(node.fqn)}
  <li>
    <div class="row" style="--depth: {depth}">
      <button
        class="twist"
        class:open
        disabled={kids.length === 0}
        aria-label={open ? "Collapse" : "Expand"}
        aria-expanded={open}
        onclick={() => toggle(node.fqn)}
      >▸</button>
      <button
        class="node kind-{node.kind}"
        class:active={node.fqn === selectedFqn}
        onclick={() => onpicknode?.(node.fqn)}
        title="{node.kind} · {node.fqn}"
      >
        <Icon class="ico" size={14} strokeWidth={1.75} aria-hidden="true" />
        <span class="label">{node.name}</span>
        {#if node.triggered}<span class="trig" title="Triggered callable">▸</span>{/if}
      </button>
    </div>
    {#if open && kids.length}
      <ul>
        {#each kids as kid}{@render row(kid, depth + 1, children)}{/each}
      </ul>
    {/if}
  </li>
{/snippet}

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

  .group { border-bottom: 1px solid var(--line); padding-bottom: 0.4rem; }
  .group:last-child { border-bottom: none; flex: 1; }
  .group-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0.7rem 0.95rem 0.4rem;
  }
  .tally {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    color: var(--ink-faint);
  }
  .empty {
    padding: 0.2rem 0.95rem 0.6rem;
    font-size: 0.76rem;
    color: var(--ink-faint);
  }
  ul { list-style: none; margin: 0; padding: 0; }
  .files, .docs { padding: 0 0.4rem; }

  /* authored doc pages (`[[doc.sidebar]]`), grouped by sidebar title */
  .doc-group-title {
    padding: 0.4rem 0.95rem 0.2rem;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.04em;
    color: var(--ink-soft);
  }
  .doc {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.45rem;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.32rem 0.5rem;
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  .doc :global(.file-ico) { flex: none; color: var(--ink-faint); }
  .doc:hover { background: var(--surface-2); color: var(--ink); }
  .doc.active { background: var(--accent-soft); color: var(--accent); }
  .doc.active :global(.file-ico) { color: var(--accent); }

  .file {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.45rem;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.34rem 0.5rem;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 0.79rem;
    font-weight: 700;
  }
  .file :global(.file-ico) { flex: none; color: var(--accent); }
  .file:hover { background: var(--surface-2); }
  .file.active { background: var(--accent-soft); color: var(--accent); }
  .file.active :global(.file-ico) { color: var(--accent); }
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

  /* the symbol hierarchy */
  .symbols { padding: 0 0.4rem; }
  .row { display: flex; align-items: center; gap: 0.1rem; padding-left: calc(var(--depth) * 0.95rem); }
  .twist {
    flex: none;
    width: 1.1rem;
    height: 1.1rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    color: var(--ink-faint);
    font-size: 0.62rem;
    cursor: pointer;
    transition: transform 0.13s, color 0.13s;
  }
  .twist.open { transform: rotate(90deg); }
  .twist:hover:not(:disabled) { color: var(--ink); }
  .twist:disabled { opacity: 0.2; cursor: default; }

  .node {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.22rem 0.5rem;
    color: var(--ink-soft);
    cursor: pointer;
  }
  .node:hover { background: var(--surface-2); color: var(--ink); }
  .node.active { background: var(--accent-soft); }
  .node.active .label { color: var(--accent); }

  .node :global(.ico) { flex: none; opacity: 0.9; }
  .label {
    font-family: var(--font-mono);
    font-size: 0.79rem;
    color: var(--ink-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .node:hover .label { color: var(--ink); }
  .trig { color: var(--accent); font-size: 0.62rem; }

  /* kind accents — icon inherits via currentColor */
  .kind-person  { color: #6e8bff; }
  .kind-system  { color: var(--accent-hi); }
  .kind-container { color: #2dd4bf; }
  .kind-component { color: #b87bf5; }
  .kind-data    { color: var(--warn); }
  .kind-callable { color: var(--ink-faint); }
</style>
