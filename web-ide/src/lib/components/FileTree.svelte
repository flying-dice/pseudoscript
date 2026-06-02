<script lang="ts">
  import { ChevronRight, File, FileCode, FileImage, FileText, Settings2 } from "@lucide/svelte";

  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";

  // One workspace file — a module, an authored doc, the manifest, or a read-only
  // companion (`other`). `relPath` is workspace-root-relative; `key` is the
  // dirty/active key (FQN or path). `binary` companions open as an inert leaf.
  type FileEntry = { key: string; kind: "module" | "doc" | "manifest" | "other"; relPath: string; label: string; fqn?: string; binary?: boolean };

  type Props = {
    entries?: FileEntry[];
    // Base-relative directories on disk, so empty folders show in the tree too.
    dirs?: string[];
    openKey?: string | null;
    errorKeys?: Set<string>;
    dirtyKeys?: Set<string>;
    onopen?: (entry: FileEntry) => void;
    // `dir` (base-relative) is the folder the action targets; omitted = root.
    oncreatefile?: (dir?: string) => void;
    oncreatedoc?: () => void;
    oncreatefolder?: (dir?: string) => void;
    onrenamefolder?: (path: string) => void;
    ondeletefolder?: (path: string) => void;
    onrenamefile?: (fqn: string) => void;
    ondeletefile?: (fqn: string) => void;
    onmovefile?: (payload: { fqn: string; destDir: string }) => void;
  };

  let {
    entries = [],
    dirs = [],
    openKey = null,
    errorKeys = new Set<string>(),
    dirtyKeys = new Set<string>(),
    onopen,
    oncreatefile,
    oncreatedoc,
    oncreatefolder,
    onrenamefolder,
    ondeletefolder,
    onrenamefile,
    onmovefile,
    ondeletefile,
  }: Props = $props();

  type TreeNode = { name: string; path: string; entry?: FileEntry; children: TreeNode[] };

  // Build a nested directory tree from the flat file list, then fold in the
  // on-disk directories so empty folders appear too.
  const tree = $derived.by<TreeNode[]>(() => {
    const root: TreeNode = { name: "", path: "", children: [] };
    const seen = new Map<string, TreeNode>([["", root]]);
    const ensureDir = (path: string): TreeNode => {
      if (seen.has(path)) return seen.get(path)!;
      const segs = path.split("/");
      const name = segs.pop()!;
      const parent = ensureDir(segs.join("/"));
      const node: TreeNode = { name, path, children: [] };
      parent.children.push(node);
      seen.set(path, node);
      return node;
    };
    for (const e of entries) {
      const segs = e.relPath.split("/");
      const fileName = segs.pop()!;
      const parent = ensureDir(segs.join("/"));
      parent.children.push({ name: fileName, path: e.relPath, entry: e, children: [] });
    }
    for (const d of dirs) if (d) ensureDir(d);
    const sort = (list: TreeNode[]): void => {
      list.sort((a, b) => (!!a.entry === !!b.entry ? a.name.localeCompare(b.name) : a.entry ? 1 : -1));
      for (const n of list) if (!n.entry) sort(n.children);
    };
    sort(root.children);
    return root.children;
  });

  // Collapsed folders, by path. Default expanded.
  let collapsed = $state(new Set<string>());
  function toggle(path: string): void {
    const next = new Set(collapsed);
    next.has(path) ? next.delete(path) : next.add(path);
    collapsed = next;
  }

  function iconFor(e: FileEntry) {
    if (e.kind === "doc") return FileText;
    if (e.kind === "manifest") return Settings2;
    if (e.kind === "other") return e.binary ? FileImage : File;
    return FileCode;
  }

  // Drag-and-drop move state (modules only).
  let dragFqn = $state<string | null>(null);
  let dropDir = $state<string | null>(null);
  function onDrop(destDir: string): void {
    if (dragFqn) onmovefile?.({ fqn: dragFqn, destDir });
    dragFqn = null;
    dropDir = null;
  }
</script>

<nav class="tree" aria-label="Workspace">
  <ContextMenu.Root>
    <ContextMenu.Trigger class="tree-trigger">
      <div class="body">
        {#if tree.length === 0}
          <div class="empty">No files yet — right-click to create one.</div>
        {:else}
          <ul>
            {#each tree as node (node.path)}{@render row(node, 0)}{/each}
          </ul>
        {/if}
      </div>
    </ContextMenu.Trigger>
    <ContextMenu.Content class="ctx-menu">
      {#if oncreatefile}<ContextMenu.Item onSelect={() => oncreatefile?.()}>New file…</ContextMenu.Item>{/if}
      {#if oncreatefolder}<ContextMenu.Item onSelect={() => oncreatefolder?.()}>New folder…</ContextMenu.Item>{/if}
      {#if oncreatedoc}<ContextMenu.Item onSelect={() => oncreatedoc?.()}>New doc…</ContextMenu.Item>{/if}
    </ContextMenu.Content>
  </ContextMenu.Root>
</nav>

{#snippet row(node: TreeNode, depth: number)}
  {#if node.entry}
    {@const e = node.entry}
    {@const Icon = iconFor(e)}
    <li>
      <ContextMenu.Root>
        <ContextMenu.Trigger class="row-trigger">
          <button
            class="file"
            class:active={e.key === openKey}
            class:has-error={errorKeys.has(e.key)}
            class:is-dirty={!errorKeys.has(e.key) && dirtyKeys.has(e.key)}
            data-testid={e.fqn ? `file-${e.fqn}` : undefined}
            style="--depth: {depth}"
            draggable={e.kind === "module" && !!onmovefile}
            ondragstart={() => (dragFqn = e.fqn ?? null)}
            ondragend={() => {
              dragFqn = null;
              dropDir = null;
            }}
            onclick={() => onopen?.(e)}
            aria-current={e.key === openKey ? "true" : undefined}
            title={e.relPath}
          >
            <Icon class="file-ico" size={14} strokeWidth={1.9} aria-hidden="true" />
            <span class="name">{node.name}</span>
            {#if dirtyKeys.has(e.key) && !errorKeys.has(e.key)}<span class="dot dirty" aria-hidden="true"></span>{/if}
            {#if errorKeys.has(e.key)}<span class="dot err" aria-hidden="true"></span>{/if}
          </button>
        </ContextMenu.Trigger>
        <ContextMenu.Content class="ctx-menu">
          <ContextMenu.Item onSelect={() => onopen?.(e)}>Open</ContextMenu.Item>
          {#if e.kind === "module"}
            {#if onrenamefile}<ContextMenu.Item onSelect={() => onrenamefile?.(e.fqn!)}>Rename…</ContextMenu.Item>{/if}
            {#if oncreatefile}<ContextMenu.Item onSelect={() => oncreatefile?.()}>New file…</ContextMenu.Item>{/if}
            {#if ondeletefile}
              <ContextMenu.Separator />
              <ContextMenu.Item variant="destructive" onSelect={() => ondeletefile?.(e.fqn!)}>Delete</ContextMenu.Item>
            {/if}
          {/if}
        </ContextMenu.Content>
      </ContextMenu.Root>
    </li>
  {:else}
    {@const open = !collapsed.has(node.path)}
    <li>
      <ContextMenu.Root>
        <ContextMenu.Trigger class="row-trigger">
          <button
            class="folder"
            class:drop={onmovefile && dropDir === node.path}
            style="--depth: {depth}"
            aria-expanded={open}
            onclick={() => toggle(node.path)}
            ondragover={(ev) => {
              if (dragFqn) {
                ev.preventDefault();
                dropDir = node.path;
              }
            }}
            ondrop={(ev) => {
              ev.preventDefault();
              onDrop(node.path);
            }}
          >
            <ChevronRight class={`twist ${open ? "open" : ""}`} size={13} strokeWidth={2.25} aria-hidden="true" />
            <span class="name dir">{node.name}</span>
          </button>
        </ContextMenu.Trigger>
        <ContextMenu.Content class="ctx-menu">
          {#if oncreatefile}<ContextMenu.Item onSelect={() => oncreatefile?.(node.path)}>New file…</ContextMenu.Item>{/if}
          {#if oncreatefolder}<ContextMenu.Item onSelect={() => oncreatefolder?.(node.path)}>New folder…</ContextMenu.Item>{/if}
          {#if onrenamefolder}<ContextMenu.Item onSelect={() => onrenamefolder?.(node.path)}>Rename folder…</ContextMenu.Item>{/if}
          {#if ondeletefolder}
            <ContextMenu.Separator />
            <ContextMenu.Item variant="destructive" onSelect={() => ondeletefolder?.(node.path)}>Delete folder</ContextMenu.Item>
          {/if}
        </ContextMenu.Content>
      </ContextMenu.Root>
      {#if open}
        <ul>
          {#each node.children as kid (kid.path)}{@render row(kid, depth + 1)}{/each}
        </ul>
      {/if}
    </li>
  {/if}
{/snippet}

<style>
  .tree {
    height: 100%;
    min-height: 0;
  }
  :global(.tree-trigger) {
    display: block;
    height: 100%;
    min-height: 0;
  }
  .body {
    height: 100%;
    min-height: 0;
    overflow: auto;
    padding: 0.3rem 0.35rem;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .empty {
    padding: 0.4rem 0.6rem;
    font-size: 0.76rem;
    color: var(--ink-faint);
  }
  :global(.row-trigger) {
    display: block;
  }
  .file,
  .folder {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.2rem 0.4rem;
    padding-left: calc(0.4rem + var(--depth) * 0.8rem);
    color: var(--ink-soft);
    cursor: pointer;
  }
  .file:hover,
  .folder:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .file.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .folder.drop {
    background: var(--accent-soft);
    outline: 1px dashed var(--accent);
  }
  .file :global(.file-ico) {
    flex: none;
    opacity: 0.85;
  }
  .file.active :global(.file-ico) {
    color: var(--accent);
  }
  .folder :global(.twist) {
    flex: none;
    color: var(--ink-faint);
    transition: transform 0.12s;
  }
  .folder :global(.twist.open) {
    transform: rotate(90deg);
  }
  .name {
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.77rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.dir {
    color: var(--ink-soft);
  }
  .dot {
    flex: none;
    margin-left: auto;
    width: 0.42rem;
    height: 0.42rem;
    border-radius: 999px;
  }
  .dot.dirty {
    background: var(--warn);
  }
  .dot.err {
    background: var(--err);
  }
</style>
