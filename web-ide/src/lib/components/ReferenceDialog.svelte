<script lang="ts">
  // The in-product language reference (Help → Language reference…): the bundled,
  // version-pinned LANG.md — the same spec the authoring-skill zip vendors —
  // rendered read-only with the Markdown live preview, so an author never leaves
  // the IDE to look up syntax (model: ide::WebIde.openLanguageReference).
  import { onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { EditorState } from "@codemirror/state";
  import { markdownLivePreview } from "$lib/markdown-live.js";
  import langMd from "$lib/bundled/LANG.md?raw";

  type Props = { onclose?: () => void };
  let { onclose }: Props = $props();

  let host = $state<HTMLElement | null>(null);

  onMount(() => {
    if (!host) return;
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: langMd,
        extensions: [
          EditorView.editable.of(false),
          EditorState.readOnly.of(true),
          EditorView.lineWrapping,
          ...markdownLivePreview(),
        ],
      }),
    });
    return () => view.destroy();
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose?.(); }}>
  <div class="sheet" role="dialog" aria-modal="true" aria-label="Language reference">
    <header class="head">
      <h1 class="title">Language reference</h1>
      <span class="sub">LANG.md — the PseudoScript spec, pinned to this build</span>
      <button class="x" data-testid="reference-close" onclick={() => onclose?.()} aria-label="Close">✕</button>
    </header>
    <div class="doc" bind:this={host} data-testid="reference-doc"></div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: grid;
    place-items: center;
    padding: 2rem;
    background: color-mix(in srgb, var(--bg) 78%, transparent);
    backdrop-filter: blur(7px) saturate(1.1);
  }
  .sheet {
    display: flex;
    flex-direction: column;
    width: min(56rem, 100%);
    height: calc(100vh - 4rem);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg), 0 0 0 1px var(--line);
    overflow: hidden;
  }
  .head {
    flex: none;
    display: flex;
    align-items: baseline;
    gap: 0.7rem;
    padding: 1rem 1.25rem 0.85rem;
    border-bottom: 1px solid var(--line);
  }
  .title {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1.02rem;
    letter-spacing: -0.02em;
  }
  .sub {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--ink-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .x {
    flex: none;
    width: 1.75rem;
    height: 1.75rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    font-size: 0.9rem;
  }
  .x:hover { background: var(--surface-2); color: var(--ink); }
  .x:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .doc {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .doc :global(.cm-editor) { height: 100%; background: transparent; }
  .doc :global(.cm-scroller) {
    overflow: auto;
    padding: 1rem 1.5rem 3rem;
    font-family: var(--font-sans);
    font-size: 0.9rem;
    line-height: 1.65;
  }
  .doc :global(.cm-content) { max-width: 46rem; caret-color: transparent; }
</style>
