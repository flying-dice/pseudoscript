<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { acceptCompletion, completionKeymap, startCompletion } from "@codemirror/autocomplete";
  import { copyLineDown, defaultKeymap, history, historyKeymap, indentWithTab, toggleComment } from "@codemirror/commands";
  import { Compartment, EditorSelection, EditorState, StateEffect, StateField, Transaction } from "@codemirror/state";
  import type { Extension, StateEffectType, Text, TransactionSpec } from "@codemirror/state";
  import { codeFolding, foldedRanges, foldEffect, foldGutter, foldKeymap, foldService, HighlightStyle, LanguageDescription, StreamLanguage, syntaxHighlighting, unfoldEffect } from "@codemirror/language";
  import { languages } from "@codemirror/language-data";
  import { toml as tomlMode } from "@codemirror/legacy-modes/mode/toml";
  import { tags as t } from "@lezer/highlight";
  import { lintGutter } from "@codemirror/lint";
  import { search, searchKeymap, openSearchPanel } from "@codemirror/search";
  import {
    Decoration,
    EditorView,
    highlightActiveLine,
    highlightActiveLineGutter,
    hoverTooltip,
    keymap,
    lineNumbers,
    ViewPlugin,
  } from "@codemirror/view";
  import type { Command, DecorationSet, Tooltip } from "@codemirror/view";
  import { pseudoscript, pseudoscriptCompletion, pseudoscriptLinter } from "$lib/pseudoscript-language.js";
  import type { CompletionFetcher } from "$lib/pseudoscript-language.js";
  import { markdownLivePreview } from "$lib/markdown-live.js";
  import type { MarkdownLivePreviewOptions } from "$lib/markdown-live.js";
  import { keybindings } from "$lib/keybindings.svelte.js";
  import { completion as symbolCompletion, definition as symbolDefinition, foldRanges, hover as symbolHover, references as symbolReferences } from "$lib/pds.js";
  import type { CompletionContext } from "@codemirror/autocomplete";
  import type { Module, Occurrence, References } from "$lib/pds.js";
  import { byteToChar, charToByte } from "$lib/offsets.js";

  /** A workspace symbol entry the host hands in; carried unmodified on `ctx`. */
  type SymbolEntry = Record<string, unknown>;

  /** The cursor location the host records / `goto` accepts: 1-based line / byte-column. */
  type Location = { line: number; col: number };

  /** The imperative API handed back to the host via {@link Props.onready}. */
  type EditorApi = {
    goto: (line: number, col: number) => void;
    location: () => Location | null;
    openSettings: () => void;
  };

  type Props = {
    value?: string;
    onchange?: (value: string) => void;
    onready?: (api: EditorApi) => void;
    modules?: Module[];
    moduleFqn?: string;
    /**
     * Stable per-file identity (fqn for modules, path for docs/manifest). Drives
     * file-switch detection and the per-file view-state cache, so cursor / scroll
     * / fold state is restored when returning to a tab. Unlike `moduleFqn` it is
     * distinct for every doc and the manifest (which all share an empty `fqn`).
     */
    fileKey?: string;
    symbols?: SymbolEntry[];
    onopensymbol?: (fqn: string) => void;
    ongotodefinition?: (fqn: string) => void;
    onnavigate?: (occ: Occurrence) => void;
    onrename?: (offset: number) => void;
    onformat?: () => void;
    onsave?: () => void;
    onopensettings?: () => void;
    /**
     * Plain-text mode: drop the PseudoScript language, completion, and linter so
     * a non-`.pds` file opens without false squiggles.
     */
    plain?: boolean;
    /**
     * Markdown mode: render the document live (Obsidian-style) instead of the
     * PseudoScript language. Implies plain (no PseudoScript features).
     */
    markdown?: boolean;
    /** TOML mode: syntax-highlight the manifest (`pds.toml`). Implies plain. */
    toml?: boolean;
    /**
     * The open file's name/path. In `plain` mode (companion files) it selects a
     * syntax-highlighting language by extension (lazily loaded), so JSON/YAML/JS
     * /CSS/… read as code rather than flat text.
     */
    filename?: string;
    /**
     * Markdown preview options: `{ resolveAsset(rel)->Promise<Blob|null>,
     * resolveLink(rel) }` for relative images / sibling-doc links (folder docs).
     */
    previewOpts?: MarkdownLivePreviewOptions;
  };

  let {
    value = "",
    onchange,
    onready,
    modules = [],
    moduleFqn = "",
    fileKey = "",
    symbols = [],
    onopensymbol,
    ongotodefinition,
    onnavigate,
    onrename,
    onformat,
    onsave,
    onopensettings,
    plain = false,
    markdown = false,
    toml = false,
    filename = "",
    previewOpts = {},
  }: Props = $props();

  // Highlight tags → the editor's --hl-* token colours, for the TOML manifest.
  const tomlHighlight = HighlightStyle.define([
    { tag: t.heading, color: "var(--hl-namespace)", fontWeight: "600" },
    { tag: t.propertyName, color: "var(--hl-property)" },
    { tag: t.keyword, color: "var(--hl-keyword)" },
    { tag: t.atom, color: "var(--hl-enum)" },
    { tag: t.string, color: "var(--hl-string)" },
    { tag: t.number, color: "var(--hl-number)" },
    { tag: t.comment, color: "var(--hl-comment)", fontStyle: "italic" },
  ]);

  let host: HTMLDivElement | undefined;
  let editor: EditorView | undefined;
  let applyingExternal = false;

  /**
   * A find-usages occurrence carrying the match-highlight offsets the dropdown
   * slices on (`match_start`/`match_end` index into `text`).
   */
  type MenuOccurrence = Occurrence & { match_start: number; match_end: number };

  /** The find-usages dropdown, anchored under the symbol, or null when closed. */
  type UsagesMenu = {
    name: string;
    total: number;
    items: MenuOccurrence[];
    top: number;
    left: number;
  };

  // The find-usages dropdown, anchored under the symbol: { name, items, top,
  // left } in viewport coords, or null when closed.
  let usagesMenu = $state<UsagesMenu | null>(null);

  // The right-click context menu, anchored at the pointer. `fqn` is the symbol
  // under the click (null elsewhere — only Format shows); `pos` re-resolves the
  // symbol for go-to-definition / find-usages. Menu element bound for focus.
  type EditorMenu = { x: number; y: number; pos: number; fqn: string | null };
  let editorMenu = $state<EditorMenu | null>(null);
  let editorMenuEl = $state<HTMLDivElement | null>(null);
  const closeEditorMenu = (): null => (editorMenu = null);
  // Run a menu action and dismiss.
  function runEditorAction(fn: () => void): void {
    fn();
    closeEditorMenu();
  }
  $effect(() => {
    if (editorMenu) editorMenuEl?.focus();
  });

  /** The live state mirror read by the once-built hover/click extensions. */
  type EditorCtx = {
    modules: Module[];
    moduleFqn: string;
    symbols: SymbolEntry[];
    onopensymbol?: (fqn: string) => void;
    ongotodefinition?: (fqn: string) => void;
    onnavigate?: (occ: Occurrence) => void;
  };

  /** A byte-char range `[from, to)` within the document. */
  type WordRange = { from: number; to: number };

  // The hover extension is built once but must read live state; this object is
  // kept current by the effect below and closed over by the tooltip source.
  const ctx: EditorCtx = { modules, moduleFqn, symbols, onopensymbol, ongotodefinition, onnavigate };
  $effect(() => {
    ctx.modules = modules;
    ctx.moduleFqn = moduleFqn;
    ctx.symbols = symbols;
    ctx.onopensymbol = onopensymbol;
    ctx.ongotodefinition = ongotodefinition;
    ctx.onnavigate = onnavigate;
  });

  // Go to the definition of the symbol at byte position `pos`: resolve it via
  // the compiler, then hand the FQN to the host (which opens the declaring file
  // and jumps). Returns whether a symbol was resolved — for the keymap/click.
  function gotoDefinition(view: EditorView, pos: number | null): boolean {
    if (pos == null) return false;
    const src = view.state.doc.toString();
    let fqn: string | null;
    try {
      fqn = symbolDefinition(ctx.modules, ctx.moduleFqn, charToByte(src, pos));
    } catch {
      return false;
    }
    if (!fqn) return false;
    ctx.ongotodefinition?.(fqn);
    return true;
  }

  // Find usages of the symbol at `pos`, opening the dropdown anchored under it.
  // Returns whether a symbol was resolved — for the keymap.
  function findUsages(view: EditorView, pos: number | null): boolean {
    if (pos == null) return false;
    const refs = resolveReferences(view, pos);
    if (!refs) return false;
    openUsagesMenu(view, pos, refs);
    return true;
  }

  // Resolve the symbol at `pos` to its workspace references (or null).
  function resolveReferences(view: EditorView, pos: number | null): References | null {
    if (pos == null) return null;
    const src = view.state.doc.toString();
    try {
      return symbolReferences(ctx.modules, ctx.moduleFqn, charToByte(src, pos));
    } catch {
      return null;
    }
  }

  // The char position of a 1-based line / byte-column in `state`.
  function posOf(state: EditorState, line: number, col: number): number {
    const ln = Math.max(1, Math.min(line, state.doc.lines));
    const lineObj = state.doc.line(ln);
    const charCol = byteToChar(lineObj.text, Math.max(0, col - 1));
    return Math.min(lineObj.from + charCol, lineObj.to);
  }

  // Open the usages dropdown anchored to the line under `anchorPos`.
  function openUsagesMenu(view: EditorView, anchorPos: number, refs: References): void {
    const c = view.coordsAtPos(anchorPos);
    if (!c) return;
    usagesMenu = {
      name: refs.fqn.split("::").at(-1) ?? refs.fqn,
      total: refs.occurrences.length,
      items: refs.occurrences as MenuOccurrence[],
      top: Math.round(c.bottom + 4),
      left: Math.round(c.left),
    };
  }
  const closeUsagesMenu = (): null => (usagesMenu = null);
  function pickUsage(occ: Occurrence): void {
    usagesMenu = null;
    ctx.onnavigate?.(occ);
  }

  // Cmd/Ctrl-click: on a symbol's own declaration, find its usages (dropdown
  // under the declaration); on any other occurrence, go to that declaration
  // (IntelliJ-style). One refs resolve serves both — it carries the
  // declaration's position and the target FQN.
  function cmdClick(view: EditorView, pos: number): boolean {
    const refs = resolveReferences(view, pos);
    if (!refs) return false;
    const decl = refs.occurrences.find(
      (o) =>
        o.decl &&
        o.fqn === ctx.moduleFqn &&
        pos >= posOf(view.state, o.line, o.col) &&
        pos <= posOf(view.state, o.end_line, o.end_col),
    );
    if (decl) openUsagesMenu(view, posOf(view.state, decl.line, decl.col), refs);
    else ctx.ongotodefinition?.(refs.fqn);
    return true;
  }

  // ── Cmd/Ctrl-hover affordance ──────────────────────────────────────────────
  // While the go-to-definition modifier is held, underline the symbol under the
  // pointer (and show a pointer cursor) when it resolves to a definition — the
  // mousedown handler does the actual jump.
  const setGotoLink: StateEffectType<WordRange | null> = StateEffect.define<WordRange | null>();
  const gotoLinkMark = Decoration.mark({ class: "cm-goto-link" });
  const gotoLinkField = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setGotoLink)) {
          deco = e.value ? Decoration.set([gotoLinkMark.range(e.value.from, e.value.to)]) : Decoration.none;
        }
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  // The identifier (including a `::` path) spanning byte-char position `pos`.
  function wordRangeAt(state: EditorState, pos: number): WordRange | null {
    const line = state.doc.lineAt(pos);
    const re = /[A-Za-z_]\w*(?:::[A-Za-z_]\w*)*/g;
    for (let m: RegExpExecArray | null; (m = re.exec(line.text)); ) {
      const from = line.from + m.index;
      const to = from + m[0].length;
      if (pos >= from && pos <= to) return { from, to };
    }
    return null;
  }

  // Whether the symbol starting at `from` resolves to a definition.
  function resolvesToDefinition(state: EditorState, from: number): boolean {
    try {
      return !!symbolDefinition(ctx.modules, ctx.moduleFqn, charToByte(state.doc.toString(), from));
    } catch {
      return false;
    }
  }

  const gotoLinkPlugin = ViewPlugin.fromClass(
    class {
      view: EditorView;
      xy: { x: number; y: number } | null;
      shown: WordRange | null;
      checked: (WordRange & { ok: boolean }) | null;
      onMove: (e: MouseEvent) => void;
      onKey: (e: KeyboardEvent) => void;
      onKeyUp: (e: KeyboardEvent) => void;
      onLeave: () => void;
      constructor(view: EditorView) {
        this.view = view;
        this.xy = null; // last pointer position
        this.shown = null; // currently underlined range
        this.checked = null; // last { from, to, ok }, to avoid re-resolving the same word
        this.onMove = (e) => {
          this.xy = { x: e.clientX, y: e.clientY };
          this.refresh(e.metaKey || e.ctrlKey);
        };
        this.onKey = (e) => {
          if (e.key === "Meta" || e.key === "Control") this.refresh(true);
        };
        this.onKeyUp = (e) => {
          if (e.key === "Meta" || e.key === "Control") this.clear();
        };
        this.onLeave = () => this.clear();
        view.dom.addEventListener("mousemove", this.onMove);
        view.dom.addEventListener("mouseleave", this.onLeave);
        window.addEventListener("keydown", this.onKey);
        window.addEventListener("keyup", this.onKeyUp);
        window.addEventListener("blur", this.onLeave);
      }
      refresh(active: boolean): void {
        if (!active || !this.xy) return this.clear();
        const pos = this.view.posAtCoords(this.xy);
        if (pos == null) return this.clear();
        const r = wordRangeAt(this.view.state, pos);
        if (!r) return this.clear();
        if (!this.checked || this.checked.from !== r.from || this.checked.to !== r.to) {
          this.checked = { ...r, ok: resolvesToDefinition(this.view.state, r.from) };
        }
        if (this.checked.ok) this.show(r);
        else this.clear();
      }
      show(r: WordRange): void {
        if (this.shown && this.shown.from === r.from && this.shown.to === r.to) return;
        this.shown = r;
        this.view.dispatch({ effects: setGotoLink.of(r) });
      }
      clear(): void {
        this.checked = null;
        if (this.shown) {
          this.shown = null;
          this.view.dispatch({ effects: setGotoLink.of(null) });
        }
      }
      destroy(): void {
        this.view.dom.removeEventListener("mousemove", this.onMove);
        this.view.dom.removeEventListener("mouseleave", this.onLeave);
        window.removeEventListener("keydown", this.onKey);
        window.removeEventListener("keyup", this.onKeyUp);
        window.removeEventListener("blur", this.onLeave);
      }
    },
  );

  const theme = EditorView.theme(
    {
      "&": { height: "100%", color: "var(--ink)", backgroundColor: "transparent", fontSize: "13.5px" },
      ".cm-content": { fontFamily: "var(--font-mono)", caretColor: "var(--accent)", padding: "0.6rem 0" },
      ".cm-gutters": {
        backgroundColor: "var(--island-bg)",
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
      ".cm-goto-link": {
        textDecoration: "underline",
        textDecorationColor: "var(--accent)",
        textUnderlineOffset: "2px",
        cursor: "pointer",
      },
      // Folded-range pill ({ ... }). CodeMirror's baseTheme paints it as a light
      // chip; override here (not in app.css) so it beats the baseTheme rule —
      // an equal-specificity app.css rule loses to CodeMirror's later-injected one.
      ".cm-foldPlaceholder": {
        background: "var(--surface-3)",
        border: "1px solid var(--line-strong)",
        color: "var(--ink-soft)",
        borderRadius: "0.25em",
        margin: "0 1px",
        padding: "0 0.35em",
      },
      // Autocomplete popup. CodeMirror's baseTheme paints a light card with a
      // bright-blue selection bar and a system font; restyle it as a
      // hairline-framed mono card on the one hot accent, matching the hover
      // card. Lives here (not app.css) for the same reason as the fold pill —
      // a JS theme rule beats CodeMirror's later-injected baseTheme; an
      // equal-specificity app.css rule does not.
      ".cm-tooltip.cm-tooltip-autocomplete": {
        border: "1px solid var(--line-strong)",
        borderRadius: "var(--radius-sm)",
        background: "var(--surface)",
        boxShadow: "var(--shadow-md)",
        overflow: "hidden",
      },
      // Two-class selector to match (and outrank, by injection order)
      // CodeMirror's baseTheme `.cm-tooltip.cm-tooltip-autocomplete > ul`,
      // which otherwise pins the list to generic `monospace`.
      ".cm-tooltip.cm-tooltip-autocomplete > ul": {
        fontFamily: "var(--font-mono)",
        fontSize: "0.74rem",
        maxHeight: "16rem",
        padding: "0.2rem",
      },
      ".cm-tooltip-autocomplete > ul > li": {
        display: "flex",
        alignItems: "baseline",
        gap: "0.5rem",
        padding: "0.22rem 0.45rem",
        borderRadius: "var(--radius-sm)",
        color: "var(--ink-soft)",
        lineHeight: "1.5",
      },
      ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
        background: "var(--accent-soft)",
        color: "var(--ink)",
      },
      ".cm-tooltip-autocomplete .cm-completionLabel": {
        flex: "1",
        minWidth: "0",
        overflow: "hidden",
        textOverflow: "ellipsis",
      },
      ".cm-tooltip-autocomplete .cm-completionMatchedText": {
        color: "var(--accent)",
        fontWeight: "600",
        textDecoration: "none",
      },
      ".cm-tooltip-autocomplete li[aria-selected] .cm-completionMatchedText": {
        color: "var(--accent-hi)",
      },
      ".cm-tooltip-autocomplete .cm-completionDetail": {
        flex: "none",
        fontStyle: "normal",
        fontSize: "0.58rem",
        letterSpacing: "0.06em",
        textTransform: "uppercase",
        color: "var(--ink-faint)",
      },
      // The type column (`icons: true`): retag CodeMirror's generic glyphs as
      // small tinted badges keyed to the completion's `type` (KIND_TYPE in
      // pseudoscript-language.js), drawn from the shared kind palette.
      ".cm-tooltip-autocomplete .cm-completionIcon": {
        flex: "none",
        width: "1.1em",
        padding: "0",
        margin: "0",
        opacity: "1",
        fontSize: "0.6rem",
        fontWeight: "700",
        textAlign: "center",
        color: "var(--ink-faint)",
      },
      ".cm-tooltip-autocomplete .cm-completionIcon::after": { content: '"·"' },
      ".cm-tooltip-autocomplete .cm-completionIcon-keyword::after": { content: '"kw"', color: "var(--accent)" },
      ".cm-tooltip-autocomplete .cm-completionIcon-class::after": { content: '"▢"', color: "var(--k-system)" },
      ".cm-tooltip-autocomplete .cm-completionIcon-type::after": { content: '"◇"', color: "var(--k-data)" },
      ".cm-tooltip-autocomplete .cm-completionIcon-function::after": { content: '"ƒ"', color: "var(--k-callable)" },
      ".cm-tooltip-autocomplete .cm-completionIcon-variable::after": { content: '"•"', color: "var(--k-person)" },
    },
    { dark: true },
  );

  // ── Folding ────────────────────────────────────────────────────────────────
  // Blocks fold by default; the navigated-to block (and its ancestors) stays
  // open. Fold extents come from the compiler's AST-accurate fold ranges, served
  // as standard LSP `FoldingRange`s (0-based line numbers), memoised per
  // document. Each maps to `{ open, close }` editor offsets: `open` on the header
  // line, `close` at the closing brace itself — past the closing line's
  // indentation, so a nested `}` folds flush against the `…` (no trailing space).
  /** A foldable block, as `{ open, close }` editor offsets. */
  type FoldBlock = { open: number; close: number };
  const rangeCache = new WeakMap<Text, FoldBlock[]>();
  function rangesOf(doc: Text): FoldBlock[] {
    let r = rangeCache.get(doc);
    if (!r) {
      r = foldRanges(doc.toString())
        .filter((range) => range.endLine > range.startLine && range.endLine < doc.lines)
        .map((range) => {
          const closeLine = doc.line(range.endLine + 1);
          const indent = closeLine.text.length - closeLine.text.trimStart().length;
          return {
            open: doc.line(range.startLine + 1).from,
            close: closeLine.from + indent,
          };
        });
      rangeCache.set(doc, r);
    }
    return r;
  }
  // The fold span for a `{ … }` pair: from the end of the opening line to the
  // closing brace, so the header line stays visible with a `…` placeholder.
  function foldSpan(doc: Text, range: FoldBlock): { from: number; to: number } | null {
    const from = doc.lineAt(range.open).to;
    return from < range.close ? { from, to: range.close } : null;
  }
  // Fold every block except those whose header line through `}` contains `pos`
  // (the target and its ancestors). `pos` null collapses everything.
  function applyFold(view: EditorView, pos: number | null): void {
    const doc = view.state.doc;
    const effects: StateEffect<{ from: number; to: number }>[] = [];
    for (const r of rangesOf(doc)) {
      const span = foldSpan(doc, r);
      if (!span) continue;
      const open = pos != null && pos >= doc.lineAt(r.open).from && pos <= r.close;
      effects.push((open ? unfoldEffect : foldEffect).of(span));
    }
    if (effects.length) view.dispatch({ effects });
  }
  // The CodeMirror fold service: a block opening on a line is foldable there.
  const pdsFoldService = foldService.of((state: EditorState, lineStart: number, lineEnd: number) => {
    for (const r of rangesOf(state.doc)) {
      if (r.open >= lineStart && r.open <= lineEnd) return foldSpan(state.doc, r);
    }
    return null;
  });

  // ── Navigation flash ─────────────────────────────────────────────────────────
  // A brief fading highlight over the lines a jump lands on (the whole block, or
  // the declaration line when it has no body). Cleared after the fade.
  const setFlash: StateEffectType<{ from: number; to: number } | null> =
    StateEffect.define<{ from: number; to: number } | null>();
  const flashField = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects) {
        if (!e.is(setFlash)) continue;
        if (!e.value) { deco = Decoration.none; continue; }
        const marks = [];
        const last = tr.state.doc.lineAt(e.value.to).number;
        for (let n = tr.state.doc.lineAt(e.value.from).number; n <= last; n++) {
          marks.push(flashLine.range(tr.state.doc.line(n).from));
        }
        deco = Decoration.set(marks);
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });
  // A line decoration on every line of the block; `cm-nav-flash` paints a
  // full-width fading wash.
  const flashLine = Decoration.line({ class: "cm-nav-flash" });
  let flashTimer: ReturnType<typeof setTimeout>;
  // The line span to flash for a jump to `pos`: the enclosing block whose header
  // is on `pos`'s line, else just that line.
  function flashSpan(doc: Text, pos: number): { from: number; to: number } {
    const line = doc.lineAt(pos);
    const block = rangesOf(doc).find((r) => doc.lineAt(r.open).number === line.number);
    return { from: line.from, to: block ? block.close : line.to };
  }

  /** Move the cursor to a 1-based line / byte-column and focus the editor,
      collapsing other blocks, flashing the target, and centring it in view. */
  function goto(line: number, col: number): void {
    if (!editor) return;
    const ln = Math.max(1, Math.min(line, editor.state.doc.lines));
    const lineObj = editor.state.doc.line(ln);
    const charCol = byteToChar(lineObj.text, Math.max(0, col - 1));
    const pos = Math.min(lineObj.from + charCol, lineObj.to);
    applyFold(editor, pos);
    editor.dispatch({
      selection: EditorSelection.cursor(pos),
      effects: [EditorView.scrollIntoView(pos, { y: "center" }), setFlash.of(flashSpan(editor.state.doc, pos))],
    });
    editor.focus();
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => editor?.dispatch({ effects: setFlash.of(null) }), 1300);
  }

  // Renders a title like ``container `sys::Web` `` with the backtick run as a
  // code span; everything goes through textContent, so model-authored names
  // cannot inject markup.
  function appendTitle(into: HTMLElement, title: string): void {
    title.split("`").forEach((part: string, i: number) => {
      const el = document.createElement(i % 2 ? "code" : "span");
      el.textContent = part;
      into.append(el);
    });
  }

  // The compiler-driven hover, served as a standard LSP `Hover` (Markdown, no
  // diagram). The actions re-resolve the symbol at the cursor via the LSP
  // `definition` so navigation and the diagram canvas still work.
  function hoverText(contents: unknown): string {
    if (typeof contents === "string") return contents;
    if (Array.isArray(contents)) return contents.map(hoverText).join("\n\n");
    return (contents as { value?: string } | null)?.value ?? "";
  }
  function symbolTooltip(view: EditorView, pos: number): Tooltip | null {
    const src = view.state.doc.toString();
    const offset = charToByte(src, pos);
    let result: ReturnType<typeof symbolHover>;
    try {
      result = symbolHover(ctx.modules, ctx.moduleFqn, offset);
    } catch {
      return null;
    }
    const value = result ? hoverText(result.contents) : "";
    if (!value) return null;
    const [head, ...rest] = value.split("\n\n");
    const titleText = head.replace(/^\*\*/, "").replace(/\*\*$/, "");
    const bodyText = rest.join("\n\n");

    return {
      pos,
      above: false,
      create() {
        const dom = document.createElement("div");
        dom.className = "pds-hover";

        const title = document.createElement("div");
        title.className = "ph-title";
        appendTitle(title, titleText);
        dom.append(title);

        if (bodyText) {
          const body = document.createElement("div");
          body.className = "ph-body";
          body.textContent = bodyText;
          dom.append(body);
        }

        // Actions (go to definition / find usages / reveal on canvas) live on the
        // right-click menu now; the hover is documentation only.
        return { dom };
      },
    };
  }

  // ── Customisable shortcuts ───────────────────────────────────────────────
  // Each catalogue command id maps to its CodeMirror command. The chords come
  // from the keybindings store (user overrides + defaults) and live in a
  // compartment so a rebind reconfigures the keymap without rebuilding the view.
  // Ordering: `acceptCompletion` returns false when no popup is open, falling
  // through to indentWithTab (added after this keymap) so Tab still indents.
  const shortcutRun: Record<string, Command> = {
    triggerAutocomplete: startCompletion,
    acceptCompletion,
    toggleComment,
    duplicateLine: copyLineDown,
    formatDocument: () => (onformat?.(), true),
    saveDocument: () => (onsave?.(), true),
    openSearch: openSearchPanel,
    goToDefinition: (v) => gotoDefinition(v, v.state.selection.main.head),
    findUsages: (v) => findUsages(v, v.state.selection.main.head),
    openSettings: () => (onopensettings?.(), true),
  };
  const shortcutKeymap = (): Extension =>
    keymap.of(Object.entries(shortcutRun).map(([id, run]) => ({ key: keybindings.keyFor(id), run })));
  const keysCompartment = new Compartment();
  // Holds the undo history so a file switch can reset it (otherwise Ctrl+Z would
  // undo across the boundary into the previous file's content).
  const historyCompartment = new Compartment();

  // Completion candidates from the shared LSP engine (via wasm), scoped to the
  // trigger before the caret. The active module uses the live document text so a
  // just-typed `.`/`::` is reflected; other modules come from `ctx.modules`.
  // Returns `[{ label, kind, detail }]`; an empty list on any wasm error so a
  // transient parse failure never breaks typing.
  const completionsAt: CompletionFetcher = (cmContext: CompletionContext) => {
    const src = cmContext.state.doc.toString();
    const offset = charToByte(src, cmContext.pos);
    const modules: Module[] = ctx.modules.map((m) =>
      m.fqn === ctx.moduleFqn ? { ...m, source: src } : m,
    );
    try {
      return symbolCompletion(modules, ctx.moduleFqn, offset);
    } catch {
      return [];
    }
  };

  // The language bundle, swapped per file type: PseudoScript (default), Markdown
  // live-preview (an authored doc), or nothing (plain text). Keeps Markdown free
  // of PseudoScript highlighting/lint while rendering it in place.
  const langCompartment = new Compartment();
  const languageBundle = (): Extension => {
    if (markdown) return markdownLivePreview(previewOpts);
    if (toml) return [StreamLanguage.define(tomlMode), syntaxHighlighting(tomlHighlight)];
    if (plain) return [];
    return [
      pseudoscript(),
      pseudoscriptCompletion(completionsAt),
      pdsFoldService,
      lintGutter(),
      pseudoscriptLinter(),
      hoverTooltip(symbolTooltip, { hoverTime: 600 }),
    ];
  };

  // Re-apply the keymap whenever a binding changes (version bumps on rebind).
  $effect(() => {
    keybindings.version;
    editor?.dispatch({ effects: keysCompartment.reconfigure(shortcutKeymap()) });
  });

  // Swap the language bundle when the file type flips (file switch) or the
  // markdown preview options change (a different doc resolves assets/links).
  $effect(() => {
    markdown;
    plain;
    toml;
    previewOpts;
    editor?.dispatch({ effects: langCompartment.reconfigure(languageBundle()) });
  });

  // Extension-based highlighting for plain companion files (JSON/YAML/JS/CSS/…),
  // layered in its own compartment so it never collides with `languageBundle`
  // (which is `[]` in plain mode). The language pack loads lazily; an unknown
  // extension (or a non-plain file) clears it back to flat text.
  const fileLangCompartment = new Compartment();
  $effect(() => {
    const name = filename;
    const isPlain = plain && !markdown && !toml;
    if (!editor) return;
    const desc = isPlain && name ? LanguageDescription.matchFilename(languages, name) : null;
    if (!desc) {
      editor.dispatch({ effects: fileLangCompartment.reconfigure([]) });
      return;
    }
    let cancelled = false;
    desc
      .load()
      .then((support) => {
        if (!cancelled) editor?.dispatch({ effects: fileLangCompartment.reconfigure(support) });
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  onMount(() => {
    editor = new EditorView({
      parent: host!,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          historyCompartment.of(history()),
          // Customisable shortcuts first (highest precedence), so a user
          // rebind beats the default bundles below. Configured in Settings;
          // see keybindings.svelte.js for the catalogue and defaults.
          keysCompartment.of(shortcutKeymap()),
          keymap.of([
            ...completionKeymap,
            ...defaultKeymap,
            ...historyKeymap,
            ...foldKeymap,
            ...searchKeymap,
            indentWithTab,
          ]),
          // The find/replace panel, docked at the top of the editor.
          search({ top: true }),
          langCompartment.of(languageBundle()),
          fileLangCompartment.of([]),
          codeFolding(),
          foldGutter(),
          flashField,
          gotoLinkField,
          gotoLinkPlugin,
          // Ctrl/Cmd-click: find usages on a declaration, else go to declaration.
          EditorView.domEventHandlers({
            mousedown(e, view) {
              if (!(e.metaKey || e.ctrlKey)) return false;
              const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
              if (pos != null && cmdClick(view, pos)) {
                e.preventDefault();
                return true;
              }
              return false;
            },
            // Right-click opens the editor menu (the native menu is suppressed
            // app-wide): Format always, plus the symbol actions when the click
            // lands on a resolvable identifier.
            contextmenu(e, view) {
              const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
              let fqn: string | null = null;
              if (pos != null) {
                try {
                  fqn = symbolDefinition(ctx.modules, ctx.moduleFqn, charToByte(view.state.doc.toString(), pos));
                } catch {
                  fqn = null;
                }
              }
              editorMenu = { x: e.clientX, y: e.clientY, pos: pos ?? 0, fqn };
              e.preventDefault();
              return true;
            },
            // Both overlays are anchored to a screen position; scrolling detaches them.
            scroll() {
              closeUsagesMenu();
              closeEditorMenu();
              return false;
            },
          }),
          theme,
          EditorView.updateListener.of((u) => {
            if (u.docChanged && !applyingExternal) onchange?.(u.state.doc.toString());
          }),
        ],
      }),
    });
    // Collapse all blocks on load — nothing is navigated to yet.
    applyFold(editor, null);
    // Expose `openSettings` so the shell can drive it from a menu if needed;
    // it requests the same shell-owned modal the keyboard command does.
    // `location()` reports the caret as a 1-based line / byte-column (the same
    // shape `goto` accepts), so the shell can record where a jump started.
    const location = (): Location | null => {
      if (!editor) return null;
      const head = editor.state.selection.main.head;
      const lineObj = editor.state.doc.lineAt(head);
      return { line: lineObj.number, col: charToByte(lineObj.text, head - lineObj.from) + 1 };
    };
    onready?.({ goto, location, openSettings: () => onopensettings?.() });
  });

  onDestroy(() => editor?.destroy());

  // Move a node to <body> so its `position: fixed` is viewport-relative — a
  // transformed/contained ancestor would otherwise offset it (CodeMirror's own
  // tooltips portal for the same reason).
  function portal(node: HTMLElement): { destroy: () => void } {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  // The cursor / scroll / fold state captured for a backgrounded file, so
  // returning to its tab lands where it was left. Keyed by `fileKey`.
  type ViewState = { selection: ReturnType<EditorSelection["toJSON"]>; scrollTop: number; folds: { from: number; to: number }[] };
  const viewStates = new Map<string, ViewState>();

  // Snapshot the live view's per-file state under `key`.
  function captureViewState(view: EditorView, key: string): void {
    const folds: { from: number; to: number }[] = [];
    foldedRanges(view.state).between(0, view.state.doc.length, (from, to) => void folds.push({ from, to }));
    viewStates.set(key, { selection: view.state.selection.toJSON(), scrollTop: view.scrollDOM.scrollTop, folds });
  }

  // Reflect an external `value` change (file switch / Format) without re-firing
  // onchange. A *file switch* (fileKey changed) also resets the undo history, so
  // Ctrl+Z can't undo back across the boundary into the previous file; a
  // same-file replace (Format) stays undoable. On switch the outgoing file's
  // view-state is saved and the incoming file's is restored (else collapsed).
  let loadedFileKey: string = fileKey;
  $effect(() => {
    const next = value;
    const key = fileKey;
    if (!editor) return;
    const switched = key !== loadedFileKey;
    if (next === editor.state.doc.toString() && !switched) return;
    if (switched) captureViewState(editor, loadedFileKey);
    const restore = switched ? viewStates.get(key) : undefined;
    applyingExternal = true;
    const len = next.length;
    const clamp = (n: number) => Math.max(0, Math.min(n, len));
    const spec: TransactionSpec = { changes: { from: 0, to: editor.state.doc.length, insert: next } };
    if (switched) {
      // Clear history (fresh `history()`); the load itself isn't an undo step.
      spec.effects = historyCompartment.reconfigure(history());
      spec.annotations = Transaction.addToHistory.of(false);
    }
    if (restore) {
      try {
        const sel = EditorSelection.fromJSON(restore.selection);
        spec.selection = EditorSelection.create(
          sel.ranges.map((r) => EditorSelection.range(clamp(r.anchor), clamp(r.head))),
          sel.mainIndex,
        );
      } catch {
        // Stale/invalid serialised selection — fall back to the default caret.
      }
    }
    editor.dispatch(spec);
    applyingExternal = false;
    loadedFileKey = key;
    usagesMenu = null; // anchor is stale after a file switch / reformat
    if (restore) {
      // Reopen the folds the file had; CodeMirror ignores spans outside the doc.
      const effects = restore.folds.map((f) => foldEffect.of({ from: clamp(f.from), to: clamp(f.to) })).filter((e) => e.value.from < e.value.to);
      if (effects.length) editor.dispatch({ effects });
      // Restore scroll after fold-driven height changes have been measured.
      const top = restore.scrollTop;
      editor.requestMeasure({ read: () => {}, write: (_m, view) => void (view.scrollDOM.scrollTop = top) });
    } else {
      // Collapse the freshly-loaded file; a queued goto then reopens its target.
      applyFold(editor, null);
    }
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key !== "Escape") return;
    if (editorMenu) closeEditorMenu();
    else if (usagesMenu) closeUsagesMenu();
  }}
/>

<div
  class="editor"
  bind:this={host}
  data-testid="editor"
  role="group"
  aria-label="PseudoScript source editor"
></div>

{#if editorMenu}
  {@const m = editorMenu}
  <div use:portal>
    <button
      class="cm-ctx-scrim"
      aria-label="Close menu"
      onclick={closeEditorMenu}
      oncontextmenu={(e) => {
        e.preventDefault();
        closeEditorMenu();
      }}
    ></button>
    <div bind:this={editorMenuEl} class="cm-ctx" role="menu" tabindex="-1" aria-label="Editor actions" style="left:{m.x}px; top:{m.y}px">
      <!-- One menu; symbol actions grey out when the click isn't on a resolvable
           identifier (JetBrains-style), so the affordances stay discoverable. -->
      <button role="menuitem" class="cm-ctx-item" disabled={!m.fqn} onclick={() => runEditorAction(() => editor && gotoDefinition(editor, m.pos))}>Go to definition</button>
      <button role="menuitem" class="cm-ctx-item" disabled={!m.fqn} onclick={() => runEditorAction(() => editor && findUsages(editor, m.pos))}>Find usages</button>
      <button role="menuitem" class="cm-ctx-item" disabled={!m.fqn} onclick={() => runEditorAction(() => m.fqn && onopensymbol?.(m.fqn))}>Reveal on canvas</button>
      <button role="menuitem" class="cm-ctx-item" disabled={!m.fqn} onclick={() => runEditorAction(() => editor && onrename?.(charToByte(editor.state.doc.toString(), m.pos)))}>Rename symbol…</button>
      <div class="cm-ctx-sep"></div>
      <button role="menuitem" class="cm-ctx-item" onclick={() => runEditorAction(() => onformat?.())}>Format document</button>
    </div>
  </div>
{/if}

{#if usagesMenu}
  <div use:portal>
    <button class="um-scrim" aria-label="Close usages" onclick={closeUsagesMenu}></button>
    <div class="usages-menu" style="top:{usagesMenu.top}px; left:{usagesMenu.left}px" role="menu">
      <div class="um-head">
        <span class="um-count">{usagesMenu.total}</span>
        usage{usagesMenu.total === 1 ? "" : "s"} of <code>{usagesMenu.name}</code>
      </div>
      {#if usagesMenu.total === 0}
        <div class="um-empty">No usages found.</div>
      {:else}
        <ul>
          {#each usagesMenu.items as occ (occ.fqn + ":" + occ.line + ":" + occ.col)}
            <li>
              <button class="um-row" role="menuitem" onclick={() => pickUsage(occ)}>
                <span class="um-loc">{occ.fqn}:{occ.line}</span>
                <span class="um-text"
                  >{occ.text.slice(0, occ.match_start)}<mark class="um-hl"
                    >{occ.text.slice(occ.match_start, occ.match_end)}</mark
                  >{occ.text.slice(occ.match_end)}</span
                >
                {#if occ.decl}<span class="um-decl">decl</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}

<style>
  .editor {
    height: 100%;
    overflow: hidden;
  }

  /* right-click context menu, anchored at the pointer */
  .cm-ctx-scrim {
    position: fixed;
    inset: 0;
    z-index: 42;
    background: transparent;
    border: none;
    cursor: default;
  }
  .cm-ctx {
    position: fixed;
    z-index: 43;
    min-width: 12rem;
    padding: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    outline: none;
  }
  .cm-ctx-item {
    display: block;
    width: 100%;
    padding: 0.4rem 0.5rem;
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    text-align: left;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    color: var(--ink-soft);
    cursor: pointer;
  }
  .cm-ctx-item:hover:not(:disabled),
  .cm-ctx-item:focus-visible:not(:disabled) {
    background: var(--surface-2);
    color: var(--ink);
    outline: none;
  }
  .cm-ctx-item:disabled {
    color: var(--ink-faint);
    opacity: 0.45;
    cursor: default;
  }
  .cm-ctx-sep {
    height: 1px;
    margin: 0.2rem 0.2rem;
    background: var(--line);
  }

  /* find-usages dropdown, anchored under the declaration */
  .um-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: none;
    cursor: default;
  }
  .usages-menu {
    position: fixed;
    z-index: 41;
    min-width: 22rem;
    max-width: min(34rem, 90vw);
    max-height: 18rem;
    overflow: auto;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
  }
  .um-head {
    position: sticky;
    top: 0;
    padding: 0.45rem 0.7rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface-3);
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.04em;
    color: var(--ink-faint);
  }
  .um-head code { color: var(--ink); }
  .um-count {
    color: var(--accent);
    font-weight: 700;
  }
  .um-empty {
    padding: 0.6rem 0.7rem;
    font-family: var(--font-mono);
    font-size: 0.74rem;
    color: var(--ink-faint);
  }
  .usages-menu ul { list-style: none; margin: 0; padding: 0.25rem; }
  .um-row {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    width: 100%;
    padding: 0.3rem 0.5rem;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    text-align: left;
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.74rem;
    cursor: pointer;
  }
  .um-row:hover { background: var(--surface-3); color: var(--ink); }
  .um-loc { flex: none; color: var(--ink-faint); font-size: 0.66rem; }
  .um-text { flex: 1; min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .um-hl {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
    border-radius: 3px;
    padding: 0 1px;
  }
  .um-decl {
    flex: none;
    color: var(--accent);
    font-size: 0.58rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
</style>
