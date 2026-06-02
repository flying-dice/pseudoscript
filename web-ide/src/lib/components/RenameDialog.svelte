<script lang="ts">
  // The rename preview: a new-name field plus every occurrence of the symbol
  // (declaration included) as a checklist, so the user reviews and deselects any
  // they don't want before applying. Built on the shadcn Dialog. Confirm hands
  // back the new name and the selected occurrence keys; the host calls the LSP
  // rename and swaps the rewritten sources into its buffers.
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import type { Occurrence, RenameSelection } from "$lib/pds.js";

  type Props = {
    symbol?: string;
    occurrences?: Occurrence[];
    onconfirm?: (newName: string, selected: RenameSelection[]) => void;
    oncancel?: () => void;
  };

  let { symbol = "", occurrences = [], onconfirm, oncancel }: Props = $props();

  const keyOf = (o: Occurrence): string => `${o.fqn}:${o.line}:${o.col}`;

  let name = $state(symbol);
  // Every occurrence starts selected; the user deselects the ones to skip.
  let chosen = $state(new Set(occurrences.map(keyOf)));

  const valid = $derived(/^[A-Za-z_]\w*$/.test(name.trim()));
  const unchanged = $derived(name.trim() === symbol);
  const selectedCount = $derived(chosen.size);
  const canApply = $derived(valid && !unchanged && selectedCount > 0);

  // Occurrences grouped by module, for a file-headed checklist.
  const groups = $derived.by(() => {
    const by = new Map<string, Occurrence[]>();
    for (const o of occurrences) {
      if (!by.has(o.fqn)) by.set(o.fqn, []);
      by.get(o.fqn)!.push(o);
    }
    return [...by.entries()].map(([fqn, items]) => ({ fqn, items }));
  });

  function toggle(o: Occurrence): void {
    const next = new Set(chosen);
    const k = keyOf(o);
    next.has(k) ? next.delete(k) : next.add(k);
    chosen = next;
  }
  const allOn = $derived(chosen.size === occurrences.length);
  function toggleAll(): void {
    chosen = allOn ? new Set() : new Set(occurrences.map(keyOf));
  }

  function apply(): void {
    if (!canApply) return;
    const selected: RenameSelection[] = occurrences
      .filter((o) => chosen.has(keyOf(o)))
      .map((o) => ({ fqn: o.fqn, line: o.line, col: o.col }));
    onconfirm?.(name.trim(), selected);
  }

  function onOpenChange(open: boolean): void {
    if (!open) oncancel?.();
  }
</script>

<Dialog.Root open onOpenChange={onOpenChange}>
  <Dialog.Content class="sm:max-w-2xl" data-testid="rename-dialog">
    <Dialog.Header>
      <Dialog.Title>Rename <code class="sym">{symbol}</code></Dialog.Title>
      <Dialog.Description>Review the {occurrences.length} occurrence{occurrences.length === 1 ? "" : "s"}; uncheck any to skip.</Dialog.Description>
    </Dialog.Header>

    <form
      class="body"
      onsubmit={(e) => {
        e.preventDefault();
        apply();
      }}
    >
      <label class="field">
        <span class="lbl">New name</span>
        <!-- svelte-ignore a11y_autofocus -->
        <Input bind:value={name} autofocus spellcheck="false" autocomplete="off" aria-label="New name" aria-invalid={!valid} class="font-mono" />
      </label>
      {#if !valid}
        <p class="err">Not a valid identifier.</p>
      {/if}

      <div class="list-head">
        <label class="all">
          <input type="checkbox" checked={allOn} onchange={toggleAll} />
          {selectedCount} of {occurrences.length} selected
        </label>
      </div>

      <div class="list">
        {#each groups as group (group.fqn)}
          <div class="grp">
            <div class="grp-head">{group.fqn}</div>
            {#each group.items as occ (keyOf(occ))}
              <label class="occ" class:off={!chosen.has(keyOf(occ))}>
                <input type="checkbox" checked={chosen.has(keyOf(occ))} onchange={() => toggle(occ)} />
                <span class="ln">:{occ.line}</span>
                <span class="preview"
                  >{occ.text.slice(0, occ.match_start)}<span class="old">{occ.text.slice(occ.match_start, occ.match_end)}</span
                  >{#if valid && !unchanged}<span class="arrow">→</span><span class="new">{name.trim()}</span>{/if}{occ.text.slice(occ.match_end)}</span
                >
                {#if occ.decl}<span class="decl">decl</span>{/if}
              </label>
            {/each}
          </div>
        {/each}
      </div>

      <Dialog.Footer>
        <Button type="button" variant="ghost" size="sm" onclick={() => oncancel?.()}>Cancel</Button>
        <Button type="submit" size="sm" disabled={!canApply}>
          Rename {selectedCount} occurrence{selectedCount === 1 ? "" : "s"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  .sym {
    font-family: var(--font-mono);
    color: var(--accent);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 0;
    /* The Dialog content is a grid; a grid item won't shrink below its content's
       min-content width by default, so a long nowrap preview line would push the
       modal wide. min-width:0 lets it shrink and the .preview ellipsis engage. */
    min-width: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .lbl {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    font-weight: 600;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .err {
    margin: 0;
    font-size: 0.76rem;
    color: var(--err);
  }
  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .all {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--ink-soft);
    cursor: pointer;
  }
  .list {
    min-width: 0;
    max-height: min(46vh, 24rem);
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .grp-head {
    position: sticky;
    top: 0;
    padding: 0.3rem 0.6rem;
    background: var(--surface-3);
    border-bottom: 1px solid var(--line);
    font-family: var(--font-mono);
    font-size: 0.64rem;
    letter-spacing: 0.04em;
    color: var(--ink-faint);
  }
  .occ {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.3rem 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.74rem;
    color: var(--ink-soft);
    cursor: pointer;
    border-bottom: 1px solid var(--line);
  }
  .occ:last-child {
    border-bottom: none;
  }
  .occ:hover {
    background: var(--surface-3);
  }
  .occ.off {
    opacity: 0.5;
  }
  .occ input,
  .all input {
    flex: none;
    accent-color: var(--accent);
    align-self: center;
  }
  .ln {
    flex: none;
    color: var(--ink-faint);
    font-size: 0.66rem;
  }
  .preview {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--ink);
  }
  .old {
    color: var(--err);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, var(--err) 60%, transparent);
  }
  .arrow {
    margin: 0 0.25rem;
    color: var(--ink-faint);
  }
  .new {
    color: var(--ok);
    font-weight: 600;
  }
  .decl {
    flex: none;
    color: var(--accent);
    font-size: 0.56rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
</style>
