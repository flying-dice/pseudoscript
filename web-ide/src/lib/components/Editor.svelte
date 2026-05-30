<script>
  import { onDestroy, onMount } from "svelte";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { EditorState } from "@codemirror/state";
  import { lintGutter } from "@codemirror/lint";
  import {
    EditorView,
    highlightActiveLine,
    highlightActiveLineGutter,
    keymap,
    lineNumbers,
  } from "@codemirror/view";
  import { pseudoscript, pseudoscriptLinter } from "$lib/pseudoscript-language.js";

  let { value = "", onchange } = $props();

  let host;
  let editor;
  let applyingExternal = false;

  const theme = EditorView.theme(
    {
      "&": { height: "100%", color: "var(--ink)", backgroundColor: "var(--surface)", fontSize: "13.5px" },
      ".cm-content": { fontFamily: "var(--font-mono)", caretColor: "var(--accent)" },
      ".cm-gutters": { backgroundColor: "var(--surface)", color: "var(--ink-faint)", border: "none" },
      ".cm-activeLine": { backgroundColor: "var(--surface-2)" },
      ".cm-activeLineGutter": { backgroundColor: "var(--surface-2)" },
      "&.cm-focused .cm-cursor": { borderLeftColor: "var(--accent)" },
      ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
        backgroundColor: "var(--accent-soft)",
      },
      ".cm-scroller": { lineHeight: "1.6" },
    },
    { dark: true },
  );

  onMount(() => {
    editor = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          pseudoscript(),
          lintGutter(),
          pseudoscriptLinter(),
          theme,
          EditorView.updateListener.of((u) => {
            if (u.docChanged && !applyingExternal) onchange?.(u.state.doc.toString());
          }),
        ],
      }),
    });
  });

  onDestroy(() => editor?.destroy());

  // Reflect an external `value` change (e.g. Format) without re-firing onchange.
  $effect(() => {
    const next = value;
    if (editor && next !== editor.state.doc.toString()) {
      applyingExternal = true;
      editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: next } });
      applyingExternal = false;
    }
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    height: 100%;
    overflow: hidden;
  }
</style>
