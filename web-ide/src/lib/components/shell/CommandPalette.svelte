<script lang="ts">
  import * as Command from "$lib/components/ui/command/index.js";

  type FileItem = { fqn: string; path: string };
  type SymbolItem = { fqn: string; name: string; kind: string };

  type Props = {
    open?: boolean;
    files?: FileItem[];
    symbols?: SymbolItem[];
    onopenfile?: (file: FileItem) => void;
    onpicksymbol?: (fqn: string) => void;
  };

  let { open = $bindable(false), files = [], symbols = [], onopenfile, onpicksymbol }: Props = $props();

  function pickFile(f: FileItem): void {
    open = false;
    onopenfile?.(f);
  }
  function pickSymbol(fqn: string): void {
    open = false;
    onpicksymbol?.(fqn);
  }
</script>

<Command.Dialog bind:open>
  <Command.Input placeholder="Go to file or symbol…" />
  <Command.List>
    <Command.Empty>No matching file or symbol.</Command.Empty>
    {#if files.length}
      <Command.Group heading="Files">
        {#each files as f (f.fqn)}
          <Command.Item value="file {f.fqn} {f.path}" onSelect={() => pickFile(f)}>
            <span class="leaf">{f.fqn}</span>
          </Command.Item>
        {/each}
      </Command.Group>
    {/if}
    {#if symbols.length}
      <Command.Group heading="Symbols">
        {#each symbols as s (s.fqn)}
          <Command.Item value="sym {s.name} {s.fqn}" onSelect={() => pickSymbol(s.fqn)}>
            <span class="kind kind-{s.kind}">{s.kind}</span>
            <span class="leaf">{s.name}</span>
            <span class="fqn">{s.fqn}</span>
          </Command.Item>
        {/each}
      </Command.Group>
    {/if}
  </Command.List>
</Command.Dialog>

<style>
  .leaf {
    font-family: var(--font-mono);
    color: var(--ink);
  }
  .kind {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-faint);
    min-width: 4.2rem;
  }
  .fqn {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--ink-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind-person {
    color: #6e8bff;
  }
  .kind-system {
    color: var(--accent-hi);
  }
  .kind-container {
    color: #2dd4bf;
  }
  .kind-component {
    color: #b87bf5;
  }
  .kind-data {
    color: var(--warn);
  }
  .kind-callable {
    color: var(--ink-faint);
  }
</style>
