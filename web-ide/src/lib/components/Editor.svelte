<script>
  import { onDestroy, onMount } from "svelte";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { EditorSelection, EditorState } from "@codemirror/state";
  import { lintGutter } from "@codemirror/lint";
  import {
    EditorView,
    highlightActiveLine,
    highlightActiveLineGutter,
    keymap,
    lineNumbers,
  } from "@codemirror/view";
  import { pseudoscript, pseudoscriptLinter } from "$lib/pseudoscript-language.js";
  import { byteToChar } from "$lib/offsets.js";

  let { value = "", onchange, onready } = $props();

  let host;
  let editor;
  let applyingExternal = false;

  const theme = EditorView.theme(
    {
      "&": { height: "100%", color: "var(--ink)", backgroundColor: "transparent", fontSize: "13.5px" },
      ".cm-content": { fontFamily: "var(--font-mono)", caretColor: "var(--accent)", padding: "0.6rem 0" },
      ".cm-gutters": {
        backgroundColor: "transparent",
        color: "var(--ink-faint)",
        border: "none",
        paddingLeft: "0.4rem",
      },
      ".cm-lineNumbers .cm-gutterElement": { padding: "0 0.7rem 0 1rem", fontSize: "0.72rem" },
      ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent) 5%, transparent)" },
      ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--accent)" },
      "&.cm-focused .cm-cursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "2px" },
      ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
        backgroundColor: "color-mix(in srgb, var(--accent) 20%, transparent)",
      },
      ".cm-scroller": { lineHeight: "1.7" },
      ".cm-lint-marker": { width: "0.8em", height: "0.8em" },
    },
    { dark: true },
  );

  /** Move the cursor to a 1-based line / byte-column and focus the editor. */
  function goto(line, col) {
    if (!editor) return;
    const ln = Math.max(1, Math.min(line, editor.state.doc.lines));
    const lineObj = editor.state.doc.line(ln);
    const charCol = byteToChar(lineObj.text, Math.max(0, col - 1));
    const pos = Math.min(lineObj.from + charCol, lineObj.to);
    editor.dispatch({ selection: EditorSelection.cursor(pos), scrollIntoView: true });
    editor.focus();
  }

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
    onready?.({ goto });
  });

  onDestroy(() => editor?.destroy());

  // Reflect an external `value` change (file switch / Format) without re-firing onchange.
  $effect(() => {
    const next = value;
    if (editor && next !== editor.state.doc.toString()) {
      applyingExternal = true;
      editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: next } });
      applyingExternal = false;
    }
  });
</script>

<div
  class="editor"
  bind:this={host}
  role="group"
  aria-label="PseudoScript source editor"
></div>

<style>
  .editor {
    height: 100%;
    overflow: hidden;
  }
</style>
