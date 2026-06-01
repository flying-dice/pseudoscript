<script lang="ts">
  import { FileCode, FileText, Pencil, Settings2, Trash2 } from "@lucide/svelte";

  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";

  // A workspace file row: its fully-qualified name and base-relative path.
  type TreeFile = {
    fqn: string;
    path: string;
  };

  // An authored doc page within a sidebar group. Extra fields (e.g. `handle`)
  // ride along untyped and are passed back verbatim through `ondocopen`.
  type DocItem = {
    title: string;
    path: string;
    [key: string]: unknown;
  };

  // A sidebar group from `[[doc.sidebar]]`.
  type DocGroup = {
    title: string;
    items: DocItem[];
  };

  // The move payload handed to `onmovefile`.
  type MovePayload = {
    file: TreeFile;
    destDir: string;
  };

  type Props = {
    workspaceName?: string;
    files?: TreeFile[];
    openPath?: string | null;
    onopen?: (file: TreeFile) => void;
    errorPaths?: Set<string>;
    // Paths whose live buffer differs from disk — render an unsaved dot (an error
    // marker takes visual precedence over the dirty one).
    dirtyPaths?: Set<string>;
    // Authored doc groups from `[[doc.sidebar]]` (`{ title, items: [{ title,
    // path }] }`), listed above Files. Clicking a page opens its raw Markdown.
    docGroups?: DocGroup[];
    ondocopen?: (item: DocItem) => void;
    // The workspace manifest path (`pds.toml`), or null when there's none. A
    // dedicated row opens it as editable raw TOML.
    manifestPath?: string | null;
    onmanifestopen?: () => void;
    // The base-relative prefix that holds modules (the manifest dir), used to
    // turn a file `path` into its base-relative directory for move targets.
    base?: string;
    // Create / FS-management actions (T9/T10/T11). Each is optional; the matching
    // affordance only renders when its callback is supplied.
    oncreatefile?: () => void;
    oncreatedoc?: () => void;
    onrenamefile?: (file: TreeFile) => void;
    onmovefile?: (payload: MovePayload) => void;
    ondeletefile?: (file: TreeFile) => void;
  };

  let {
    workspaceName = "",
    files = [],
    openPath = null,
    onopen,
    errorPaths = new Set<string>(),
    dirtyPaths = new Set<string>(),
    docGroups = [],
    ondocopen,
    manifestPath = null,
    onmanifestopen,
    base = "",
    oncreatefile,
    oncreatedoc,
    onrenamefile,
    onmovefile, // ({ file, destDir })
    ondeletefile,
  }: Props = $props();

  // A file's directory, base-relative ("" = workspace root).
  function dirOf(file: TreeFile): string {
    const rel = base && file.path.startsWith(`${base}/`) ? file.path.slice(base.length + 1) : file.path;
    const i = rel.lastIndexOf("/");
    return i === -1 ? "" : rel.slice(0, i);
  }

  // Group files by base-relative directory so move has real drop targets — root
  // files first, then each subdirectory in path order.
  const fileGroups = $derived.by(() => {
    const byDir = new Map<string, TreeFile[]>();
    for (const f of files) {
      const d = dirOf(f);
      if (!byDir.has(d)) byDir.set(d, []);
      byDir.get(d)!.push(f);
    }
    const dirs = [...byDir.keys()].sort((a, b) => (a === "" ? -1 : b === "" ? 1 : a.localeCompare(b)));
    return dirs.map((dir) => ({ dir, items: byDir.get(dir)! }));
  });

  // Drag-and-drop move state: the dragged file and the hovered drop dir.
  let dragFile = $state<TreeFile | null>(null);
  let dropDir = $state<string | null>(null);

  function onDrop(destDir: string): void {
    if (dragFile && dirOf(dragFile) !== destDir) onmovefile?.({ file: dragFile, destDir });
    dragFile = null;
    dropDir = null;
  }
</script>

<nav class="tree" aria-label="Workspace">
  <div class="head">
    <span class="kicker">Workspace</span>
    <span class="name" title={workspaceName}>{workspaceName}</span>
  </div>

  {#if docGroups.length || oncreatedoc}
    <section class="group">
      <div class="group-head">
        <span class="kicker">Documentation</span>
        {#if oncreatedoc}
          <button class="add" title="New doc page" aria-label="New doc page" onclick={() => oncreatedoc?.()}>+</button>
        {/if}
      </div>
      {#each docGroups as group}
        <div class="doc-group-title">{group.title}</div>
        <ul class="docs">
          {#each group.items as item}
            <li>
              <button
                class="doc"
                class:active={item.path === openPath}
                class:is-dirty={dirtyPaths.has(item.path)}
                onclick={() => ondocopen?.(item)}
                aria-current={item.path === openPath ? "true" : undefined}
                title={dirtyPaths.has(item.path) ? `${item.path} · unsaved changes` : item.path}
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

  {#if manifestPath}
    <section class="group">
      <div class="group-head"><span class="kicker">Manifest</span></div>
      <ul class="files">
        <li>
          <button
            class="file"
            class:active={manifestPath === openPath}
            class:is-dirty={dirtyPaths.has(manifestPath)}
            onclick={() => onmanifestopen?.()}
            aria-current={manifestPath === openPath ? "true" : undefined}
            title={dirtyPaths.has(manifestPath) ? `${manifestPath} · unsaved changes` : manifestPath}
          >
            <Settings2 class="file-ico" size={15} strokeWidth={2} aria-hidden="true" />
            <span class="fqn">pds.toml</span>
          </button>
        </li>
      </ul>
    </section>
  {/if}

  <section class="group">
    <div class="group-head">
      <span class="kicker">Files</span>
      <span class="tally">{files.length}</span>
      {#if oncreatefile}
        <button class="add" title="New .pds file" aria-label="New .pds file" onclick={() => oncreatefile?.()}>+</button>
      {/if}
    </div>
    {#if files.length === 0}
      <div class="empty">No <code>.pds</code> modules here.</div>
    {:else}
      {#each fileGroups as fgroup}
        {#if fgroup.dir}
          <div
            class="dir-row"
            class:drop={onmovefile && dropDir === fgroup.dir}
            role="group"
            ondragover={(e) => {
              if (dragFile) {
                e.preventDefault();
                dropDir = fgroup.dir;
              }
            }}
            ondragleave={() => {
              if (dropDir === fgroup.dir) dropDir = null;
            }}
            ondrop={(e) => {
              e.preventDefault();
              onDrop(fgroup.dir);
            }}
          >{fgroup.dir}/</div>
        {/if}
        <ul
          class="files"
          class:drop={onmovefile && dropDir === fgroup.dir}
          ondragover={(e) => {
            if (dragFile) {
              e.preventDefault();
              dropDir = fgroup.dir;
            }
          }}
          ondrop={(e) => {
            e.preventDefault();
            onDrop(fgroup.dir);
          }}
        >
          {#each fgroup.items as file}
            <li>
              <ContextMenu.Root>
                <ContextMenu.Trigger class="row-trigger">
                  <div class="file-row">
                    <button
                      class="file"
                      class:active={file.path === openPath}
                      class:has-error={errorPaths.has(file.path)}
                      class:is-dirty={!errorPaths.has(file.path) && dirtyPaths.has(file.path)}
                      data-testid="file-{file.fqn}"
                      draggable={!!onmovefile}
                      ondragstart={() => (dragFile = file)}
                      ondragend={() => {
                        dragFile = null;
                        dropDir = null;
                      }}
                      onclick={() => onopen?.(file)}
                      aria-current={file.path === openPath ? "true" : undefined}
                      title={dirtyPaths.has(file.path) ? `${file.path} · unsaved changes` : file.path}
                    >
                      <FileCode class="file-ico" size={15} strokeWidth={2} aria-hidden="true" />
                      <span class="fqn">{file.fqn}</span>
                    </button>
                    {#if onrenamefile || ondeletefile}
                      <span class="row-actions">
                        {#if onrenamefile}
                          <button class="act" title="Rename" aria-label="Rename {file.fqn}" onclick={(e) => { e.stopPropagation(); onrenamefile?.(file); }}>
                            <Pencil size={13} strokeWidth={2} aria-hidden="true" />
                          </button>
                        {/if}
                        {#if ondeletefile}
                          <button class="act danger" title="Delete" aria-label="Delete {file.fqn}" onclick={(e) => { e.stopPropagation(); ondeletefile?.(file); }}>
                            <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
                          </button>
                        {/if}
                      </span>
                    {/if}
                  </div>
                </ContextMenu.Trigger>
                <ContextMenu.Content class="ctx-menu">
                  <ContextMenu.Item onSelect={() => onopen?.(file)}>Open</ContextMenu.Item>
                  {#if onrenamefile}<ContextMenu.Item onSelect={() => onrenamefile?.(file)}>Rename…</ContextMenu.Item>{/if}
                  {#if oncreatefile}<ContextMenu.Item onSelect={() => oncreatefile?.()}>New file…</ContextMenu.Item>{/if}
                  {#if ondeletefile}
                    <ContextMenu.Separator />
                    <ContextMenu.Item variant="destructive" onSelect={() => ondeletefile?.(file)}>Delete</ContextMenu.Item>
                  {/if}
                </ContextMenu.Content>
              </ContextMenu.Root>
            </li>
          {/each}
        </ul>
      {/each}
    {/if}
  </section>
</nav>

<style>
  .tree {
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  :global(.row-trigger) {
    display: block;
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

  /* "+" create affordance in a section header */
  .add {
    margin-left: auto;
    width: 1.25rem;
    height: 1.25rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    font-size: 0.85rem;
    line-height: 1;
    cursor: pointer;
  }
  .add:hover { background: var(--surface-2); color: var(--accent); border-color: var(--accent); }

  /* a base-relative subdirectory label / drop target */
  .dir-row {
    padding: 0.3rem 0.95rem 0.15rem;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.04em;
    color: var(--ink-faint);
    border-radius: var(--radius-sm);
  }
  .dir-row.drop, .files.drop { background: var(--accent-soft); }

  /* file row: the open button plus hover-revealed rename/delete actions */
  .file-row { display: flex; align-items: center; }
  .file-row .file { flex: 1; min-width: 0; }
  .row-actions {
    display: flex;
    gap: 0.1rem;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .file-row:hover .row-actions, .file-row:focus-within .row-actions { opacity: 1; }
  .act {
    width: 1.4rem;
    height: 1.4rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    cursor: pointer;
  }
  .act:hover { background: var(--surface-2); color: var(--ink); }
  .act.danger:hover { color: var(--err); }

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
  /* unsaved marker — mirrors the error dot, in the warn colour. The error dot
     wins (is-dirty is only set when has-error isn't). */
  .file.is-dirty .fqn::after,
  .doc.is-dirty .fqn::after {
    content: "";
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-left: 0.45rem;
    border-radius: 50%;
    background: var(--warn);
    vertical-align: middle;
  }
  .fqn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
