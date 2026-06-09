<script lang="ts">
  import * as Command from "$lib/components/ui/command/index.js";

  type FileItem = { fqn: string; path: string };
  type SymbolItem = { fqn: string; name: string; kind: string };
  type Module = { fqn: string; source: string };
  type TextMatch = { fqn: string; line: number; text: string };
  // An IDE command surfaced in the "Actions" tab; `keywords` widens fuzzy matching.
  type ActionItem = { id: string; label: string; hint?: string; keywords?: string; run: () => void };

  type Mode = "all" | "types" | "files" | "symbols" | "actions" | "text";

  type Props = {
    open?: boolean;
    files?: FileItem[];
    symbols?: SymbolItem[];
    modules?: Module[];
    actions?: ActionItem[];
    onopenfile?: (file: FileItem) => void;
    onpicksymbol?: (fqn: string) => void;
    onopentext?: (fqn: string, line: number) => void;
  };

  let {
    open = $bindable(false),
    files = [],
    symbols = [],
    modules = [],
    actions = [],
    onopenfile,
    onpicksymbol,
    onopentext,
  }: Props = $props();

  // IntelliJ's tab order: Search Everywhere opens on "All" and Tab cycles through.
  const TABS: { id: Mode; label: string }[] = [
    { id: "all", label: "All" },
    { id: "types", label: "Types" },
    { id: "files", label: "Files" },
    { id: "symbols", label: "Symbols" },
    { id: "actions", label: "Actions" },
    { id: "text", label: "Text" },
  ];
  const PLACEHOLDER: Record<Mode, string> = {
    all: "Search everywhere…",
    types: "Go to type…",
    files: "Go to file…",
    symbols: "Go to symbol…",
    actions: "Run an action…",
    text: "Search file contents…",
  };

  // "Types" are the structural / data nodes (the C4 boxes + data shapes); callables
  // and anything else stay symbol-only.
  const TYPE_KINDS = new Set(["person", "system", "container", "component", "data", "feature"]);

  let mode = $state<Mode>("all");
  let query = $state("");
  // The query the result lists actually filter on, debounced off `query`. Typing
  // updates the input instantly; the (capped) lists recompute one tick later, so a
  // fast typist isn't re-filtering a large model on every keystroke.
  let q = $state("");
  let inputEl = $state<HTMLInputElement | null>(null);

  // Reset to "All" each time the palette opens. The input owns focus (the
  // `autofocus` prop on Command.Input, which re-fires as the dialog content
  // remounts) so ↑/↓ drive the list and Enter selects (native cmdk behaviour).
  $effect(() => {
    if (open) {
      mode = "all";
      query = "";
      q = "";
    }
  });

  // Debounce the typed query into `q` (90ms). Cleared synchronously when emptied
  // so clearing the box feels instant.
  $effect(() => {
    const next = query;
    if (next === "") {
      q = "";
      return;
    }
    const t = setTimeout(() => (q = next), 90);
    return () => clearTimeout(t);
  });

  // At most this many rows per group reach the DOM — a "search everywhere" over a
  // large model otherwise renders thousands of nodes and janks on every keystroke.
  const CAP = 50;
  // Filter `items` by a case-insensitive substring of the key, ranked by match
  // position (earlier = better), capped. An empty query returns the first CAP.
  function filter<T>(items: T[], key: (it: T) => string, on: boolean): T[] {
    if (!on) return [];
    if (!q) return items.slice(0, CAP);
    const needle = q.toLowerCase();
    const hits: { it: T; at: number }[] = [];
    for (const it of items) {
      const at = key(it).toLowerCase().indexOf(needle);
      if (at !== -1) hits.push({ it, at });
    }
    hits.sort((a, b) => a.at - b.at);
    return hits.slice(0, CAP).map((h) => h.it);
  }

  const typeSymbols = $derived(symbols.filter((s) => TYPE_KINDS.has(s.kind)));

  // Which sources a tab shows. "All" unions files + symbols + actions (types are a
  // subset of symbols, so they aren't repeated).
  const showFiles = $derived(mode === "all" || mode === "files");
  const showSymbols = $derived(mode === "all" || mode === "symbols");
  const showTypes = $derived(mode === "types");
  const showActions = $derived(mode === "all" || mode === "actions");

  // The capped, filtered rows per group. cmdk's own filtering is off
  // (`shouldFilter={false}`); these lists are the visible set, so keyboard nav and
  // Enter operate over exactly what's shown.
  const fFiles = $derived(filter(files, (f) => `${f.fqn} ${f.path}`, showFiles));
  const fSymbols = $derived(filter(symbols, (s) => `${s.name} ${s.fqn}`, showSymbols));
  const fTypes = $derived(filter(typeSymbols, (s) => `${s.name} ${s.fqn}`, showTypes));
  const fActions = $derived(filter(actions, (a) => `${a.label} ${a.keywords ?? ""}`, showActions));

  // Text mode greps module sources directly (capped, debounced via `q`).
  const MAX_TEXT = 200;
  const textMatches = $derived.by<TextMatch[]>(() => {
    const needle = q.trim().toLowerCase();
    if (mode !== "text" || needle.length < 2) return [];
    const out: TextMatch[] = [];
    for (const m of modules) {
      const lines = m.source.split("\n");
      for (let i = 0; i < lines.length; i++) {
        if (lines[i].toLowerCase().includes(needle)) {
          out.push({ fqn: m.fqn, line: i + 1, text: lines[i].trim() });
          if (out.length >= MAX_TEXT) return out;
        }
      }
    }
    return out;
  });

  function selectTab(next: Mode): void {
    mode = next;
    inputEl?.focus();
  }
  // Tab / Shift-Tab cycle the tabs without leaving the input (IntelliJ style).
  // stopPropagation keeps the event from reaching the dialog's focus trap, which
  // would otherwise move focus to the next focusable element off the input.
  function onKeydown(e: KeyboardEvent): void {
    if (e.key !== "Tab") return;
    e.preventDefault();
    e.stopPropagation();
    const i = TABS.findIndex((t) => t.id === mode);
    const next = (i + (e.shiftKey ? -1 : 1) + TABS.length) % TABS.length;
    selectTab(TABS[next].id);
  }

  function pickFile(f: FileItem): void {
    open = false;
    onopenfile?.(f);
  }
  function pickSymbol(fqn: string): void {
    open = false;
    onpicksymbol?.(fqn);
  }
  function pickText(m: TextMatch): void {
    open = false;
    onopentext?.(m.fqn, m.line);
  }
  function pickAction(a: ActionItem): void {
    open = false;
    a.run();
  }
</script>

<Command.Dialog bind:open shouldFilter={false} class="w-[min(56rem,92vw)] sm:max-w-[56rem]">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div onkeydown={onKeydown} data-testid="command-palette">
    <div class="modes" role="tablist" aria-label="Search scope">
      {#each TABS as t (t.id)}
        <button class="mode" class:on={mode === t.id} role="tab" aria-selected={mode === t.id} data-testid="command-mode-{t.id}" onclick={() => selectTab(t.id)}>
          {t.label}
        </button>
      {/each}
    </div>
    <Command.Input autofocus bind:value={query} bind:ref={inputEl} placeholder={PLACEHOLDER[mode]} data-testid="command-input" />
    <Command.List>
      <Command.Empty>{mode === "text" && q.trim().length < 2 ? "Type at least 2 characters." : "No matches."}</Command.Empty>

      {#if mode === "text"}
        <Command.Group heading={textMatches.length ? `${textMatches.length}${textMatches.length >= MAX_TEXT ? "+" : ""} matches` : "Text"}>
          {#each textMatches as m, i (m.fqn + ":" + m.line + ":" + i)}
            <Command.Item value="text {m.fqn} {m.text}" onSelect={() => pickText(m)}>
              <span class="fqn-line">{m.fqn}:{m.line}</span>
              <span class="snippet">{m.text}</span>
            </Command.Item>
          {/each}
        </Command.Group>
      {:else}
        {#if fTypes.length}
          <Command.Group heading={fTypes.length >= CAP ? `Types (${CAP}+)` : "Types"}>
            {#each fTypes as s (s.fqn)}
              <Command.Item value="type {s.fqn}" onSelect={() => pickSymbol(s.fqn)}>
                <span class="kind kind-{s.kind}">{s.kind}</span>
                <span class="leaf">{s.name}</span>
                <span class="fqn">{s.fqn}</span>
              </Command.Item>
            {/each}
          </Command.Group>
        {/if}
        {#if fFiles.length}
          <Command.Group heading={fFiles.length >= CAP ? `Files (${CAP}+)` : "Files"}>
            {#each fFiles as f (f.fqn)}
              <Command.Item value="file {f.fqn}" data-testid="cmd-file-{f.fqn}" onSelect={() => pickFile(f)}>
                <span class="leaf">{f.fqn}</span>
              </Command.Item>
            {/each}
          </Command.Group>
        {/if}
        {#if fSymbols.length}
          <Command.Group heading={fSymbols.length >= CAP ? `Symbols (${CAP}+)` : "Symbols"}>
            {#each fSymbols as s (s.fqn)}
              <Command.Item value="sym {s.fqn}" data-testid="cmd-sym-{s.fqn}" onSelect={() => pickSymbol(s.fqn)}>
                <span class="kind kind-{s.kind}">{s.kind}</span>
                <span class="leaf">{s.name}</span>
                <span class="fqn">{s.fqn}</span>
              </Command.Item>
            {/each}
          </Command.Group>
        {/if}
        {#if fActions.length}
          <Command.Group heading="Actions">
            {#each fActions as a (a.id)}
              <Command.Item value="action {a.id}" data-testid="cmd-action-{a.id}" onSelect={() => pickAction(a)}>
                <span class="leaf">{a.label}</span>
                {#if a.hint}<span class="fqn">{a.hint}</span>{/if}
              </Command.Item>
            {/each}
          </Command.Group>
        {/if}
      {/if}
    </Command.List>
  </div>
</Command.Dialog>

<style>
  .modes {
    display: flex;
    gap: 0.2rem;
    padding: 0.5rem 0.6rem 0;
  }
  .mode {
    padding: 0.2rem 0.6rem;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    font-family: var(--font-sans);
    font-size: 0.74rem;
    cursor: pointer;
  }
  .mode:hover {
    color: var(--ink);
  }
  .mode.on {
    background: var(--surface-3);
    color: var(--ink);
    border-color: var(--line-strong);
  }
  /* Drop the app-wide accent focus ring on the search box — the dialog already
     frames it, and the orange glow reads as an error state here. */
  :global([data-slot="command-input"]:focus-visible) {
    outline: none;
  }
  /* The active row (keyboard cursor). cmdk marks it `data-selected` on the
     item; the default `bg-muted` is invisible on the popover, so the arrows
     looked dead — give it a clean, clearly-visible surface highlight. */
  :global([data-slot="command-item"][data-selected]) {
    background: var(--surface-3);
  }
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
  .fqn-line {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--accent);
    min-width: 9rem;
  }
  .snippet {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink-soft);
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
  .kind-feature {
    color: #34d399;
  }
</style>
