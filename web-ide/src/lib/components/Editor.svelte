<script>
  import { onDestroy, onMount } from "svelte";
  import { acceptCompletion, completionKeymap, startCompletion } from "@codemirror/autocomplete";
  import { copyLineDown, defaultKeymap, history, historyKeymap, indentWithTab, toggleComment } from "@codemirror/commands";
  import { Compartment, EditorSelection, EditorState, StateEffect, StateField } from "@codemirror/state";
  import { codeFolding, foldEffect, foldGutter, foldKeymap, foldService, unfoldEffect } from "@codemirror/language";
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
  import { pseudoscript, pseudoscriptCompletion, pseudoscriptLinter } from "$lib/pseudoscript-language.js";
  import { markdownLivePreview } from "$lib/markdown-live.js";
  import { keybindings } from "$lib/keybindings.svelte.js";
  import { definition as symbolDefinition, hover as symbolHover, references as symbolReferences } from "$lib/pds.js";
  import { byteToChar, charToByte } from "$lib/offsets.js";
  import { blockRanges } from "$lib/blocks.js";

  let {
    value = "",
    onchange,
    onready,
    modules = [],
    moduleFqn = "",
    symbols = [],
    onopensymbol,
    ongotodefinition,
    onnavigate,
    onformat,
    onsave,
    onopensettings,
    // Plain-text mode: drop the PseudoScript language, completion, and linter so
    // a non-`.pds` file opens without false squiggles.
    plain = false,
    // Markdown mode: render the document live (Obsidian-style) instead of the
    // PseudoScript language. Implies plain (no PseudoScript features).
    markdown = false,
    // Markdown preview options: `{ resolveAsset(rel)->Promise<Blob|null>,
    // resolveLink(rel) }` for relative images / sibling-doc links (folder docs).
    previewOpts = {},
  } = $props();

  let host;
  let editor;
  let applyingExternal = false;

  // The find-usages dropdown, anchored under the symbol: { name, items, top,
  // left } in viewport coords, or null when closed.
  let usagesMenu = $state(null);

  // The hover extension is built once but must read live state; this object is
  // kept current by the effect below and closed over by the tooltip source.
  const ctx = { modules, moduleFqn, symbols, onopensymbol, ongotodefinition, onnavigate };
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
  function gotoDefinition(view, pos) {
    if (pos == null) return false;
    const src = view.state.doc.toString();
    let fqn;
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
  function findUsages(view, pos) {
    if (pos == null) return false;
    const refs = resolveReferences(view, pos);
    if (!refs) return false;
    openUsagesMenu(view, pos, refs);
    return true;
  }

  // Resolve the symbol at `pos` to its workspace references (or null).
  function resolveReferences(view, pos) {
    if (pos == null) return null;
    const src = view.state.doc.toString();
    try {
      return symbolReferences(ctx.modules, ctx.moduleFqn, charToByte(src, pos));
    } catch {
      return null;
    }
  }

  // The char position of a 1-based line / byte-column in `state`.
  function posOf(state, line, col) {
    const ln = Math.max(1, Math.min(line, state.doc.lines));
    const lineObj = state.doc.line(ln);
    const charCol = byteToChar(lineObj.text, Math.max(0, col - 1));
    return Math.min(lineObj.from + charCol, lineObj.to);
  }

  // Open the usages dropdown anchored to the line under `anchorPos`.
  function openUsagesMenu(view, anchorPos, refs) {
    const c = view.coordsAtPos(anchorPos);
    if (!c) return;
    usagesMenu = {
      name: refs.fqn.split("::").at(-1),
      total: refs.occurrences.length,
      items: refs.occurrences,
      top: Math.round(c.bottom + 4),
      left: Math.round(c.left),
    };
  }
  const closeUsagesMenu = () => (usagesMenu = null);
  function pickUsage(occ) {
    usagesMenu = null;
    ctx.onnavigate?.(occ);
  }

  // Cmd/Ctrl-click: on a symbol's own declaration, find its usages (dropdown
  // under the declaration); on any other occurrence, go to that declaration
  // (IntelliJ-style). One refs resolve serves both — it carries the
  // declaration's position and the target FQN.
  function cmdClick(view, pos) {
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
  const setGotoLink = StateEffect.define();
  const gotoLinkMark = Decoration.mark({ class: "cm-goto-link" });
  const gotoLinkField = StateField.define({
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
  function wordRangeAt(state, pos) {
    const line = state.doc.lineAt(pos);
    const re = /[A-Za-z_]\w*(?:::[A-Za-z_]\w*)*/g;
    for (let m; (m = re.exec(line.text)); ) {
      const from = line.from + m.index;
      const to = from + m[0].length;
      if (pos >= from && pos <= to) return { from, to };
    }
    return null;
  }

  // Whether the symbol starting at `from` resolves to a definition.
  function resolvesToDefinition(state, from) {
    try {
      return !!symbolDefinition(ctx.modules, ctx.moduleFqn, charToByte(state.doc.toString(), from));
    } catch {
      return false;
    }
  }

  const gotoLinkPlugin = ViewPlugin.fromClass(
    class {
      constructor(view) {
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
      refresh(active) {
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
      show(r) {
        if (this.shown && this.shown.from === r.from && this.shown.to === r.to) return;
        this.shown = r;
        this.view.dispatch({ effects: setGotoLink.of(r) });
      }
      clear() {
        this.checked = null;
        if (this.shown) {
          this.shown = null;
          this.view.dispatch({ effects: setGotoLink.of(null) });
        }
      }
      destroy() {
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
        boxShadow: "0 18px 40px rgba(0, 0, 0, 0.45)",
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
  // open. Fold extents come from `blockRanges`, memoised per document.
  const rangeCache = new WeakMap();
  function rangesOf(doc) {
    let r = rangeCache.get(doc);
    if (!r) rangeCache.set(doc, (r = blockRanges(doc.toString())));
    return r;
  }
  // The fold span for a `{ … }` pair: from the end of the opening line to the
  // closing brace, so the header line stays visible with a `…` placeholder.
  function foldSpan(doc, range) {
    const from = doc.lineAt(range.open).to;
    return from < range.close ? { from, to: range.close } : null;
  }
  // Fold every block except those whose header line through `}` contains `pos`
  // (the target and its ancestors). `pos` null collapses everything.
  function applyFold(view, pos) {
    const doc = view.state.doc;
    const effects = [];
    for (const r of rangesOf(doc)) {
      const span = foldSpan(doc, r);
      if (!span) continue;
      const open = pos != null && pos >= doc.lineAt(r.open).from && pos <= r.close;
      effects.push((open ? unfoldEffect : foldEffect).of(span));
    }
    if (effects.length) view.dispatch({ effects });
  }
  // The CodeMirror fold service: a block opening on a line is foldable there.
  const pdsFoldService = foldService.of((state, lineStart, lineEnd) => {
    for (const r of rangesOf(state.doc)) {
      if (r.open >= lineStart && r.open <= lineEnd) return foldSpan(state.doc, r);
    }
    return null;
  });

  // ── Navigation flash ─────────────────────────────────────────────────────────
  // A brief fading highlight over the lines a jump lands on (the whole block, or
  // the declaration line when it has no body). Cleared after the fade.
  const setFlash = StateEffect.define();
  const flashField = StateField.define({
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
  let flashTimer;
  // The line span to flash for a jump to `pos`: the enclosing block whose header
  // is on `pos`'s line, else just that line.
  function flashSpan(doc, pos) {
    const line = doc.lineAt(pos);
    const block = rangesOf(doc).find((r) => doc.lineAt(r.open).number === line.number);
    return { from: line.from, to: block ? block.close : line.to };
  }

  /** Move the cursor to a 1-based line / byte-column and focus the editor,
      collapsing other blocks, flashing the target, and centring it in view. */
  function goto(line, col) {
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
  function appendTitle(into, title) {
    title.split("`").forEach((part, i) => {
      const el = document.createElement(i % 2 ? "code" : "span");
      el.textContent = part;
      into.append(el);
    });
  }

  // The compiler-driven hover: symbol info plus its fitting diagram, with
  // actions to dock the diagram in the side panel or open it full-screen. The
  // IDE never decides which diagram a symbol gets — `symbolHover` (WASM) does.
  function symbolTooltip(view, pos) {
    const src = view.state.doc.toString();
    const offset = charToByte(src, pos);
    let result;
    try {
      result = symbolHover(ctx.modules, ctx.moduleFqn, offset);
    } catch {
      return null;
    }
    if (!result) return null;

    return {
      pos,
      above: false,
      create() {
        const dom = document.createElement("div");
        dom.className = "pds-hover";

        const title = document.createElement("div");
        title.className = "ph-title";
        appendTitle(title, result.info.title);
        dom.append(title);

        if (result.info.body) {
          const body = document.createElement("div");
          body.className = "ph-body";
          body.textContent = result.info.body;
          dom.append(body);
        }

        const actions = document.createElement("div");
        actions.className = "ph-actions";
        const button = (label, fn) => {
          const b = document.createElement("button");
          b.type = "button";
          b.textContent = label;
          b.addEventListener("mousedown", (e) => {
            e.preventDefault();
            fn?.(result.info);
          });
          actions.append(b);
        };
        button("Go to definition", () => ctx.ongotodefinition?.(result.info.fqn));
        button("Find usages", () => findUsages(view, pos));
        button("Canvas", ctx.onopensymbol);
        dom.append(actions);

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
  const shortcutRun = {
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
  const shortcutKeymap = () =>
    keymap.of(Object.entries(shortcutRun).map(([id, run]) => ({ key: keybindings.keyFor(id), run })));
  const keysCompartment = new Compartment();

  // The language bundle, swapped per file type: PseudoScript (default), Markdown
  // live-preview (an authored doc), or nothing (plain text). Keeps Markdown free
  // of PseudoScript highlighting/lint while rendering it in place.
  const langCompartment = new Compartment();
  const languageBundle = () => {
    if (markdown) return markdownLivePreview(previewOpts);
    if (plain) return [];
    return [
      pseudoscript(),
      pseudoscriptCompletion(() => ctx.symbols),
      pdsFoldService,
      lintGutter(),
      pseudoscriptLinter(),
      hoverTooltip(symbolTooltip, { hoverTime: 250 }),
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
    previewOpts;
    editor?.dispatch({ effects: langCompartment.reconfigure(languageBundle()) });
  });

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
            // The dropdown is anchored to a screen position; scrolling detaches it.
            scroll() {
              closeUsagesMenu();
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
    onready?.({ goto, openSettings: () => onopensettings?.() });
  });

  onDestroy(() => editor?.destroy());

  // Move a node to <body> so its `position: fixed` is viewport-relative — a
  // transformed/contained ancestor would otherwise offset it (CodeMirror's own
  // tooltips portal for the same reason).
  function portal(node) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  // Reflect an external `value` change (file switch / Format) without re-firing onchange.
  $effect(() => {
    const next = value;
    if (editor && next !== editor.state.doc.toString()) {
      applyingExternal = true;
      editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: next } });
      applyingExternal = false;
      usagesMenu = null; // anchor is stale after a file switch / reformat
      // Collapse the freshly-loaded file; a queued goto then reopens its target.
      applyFold(editor, null);
    }
  });
</script>

<svelte:window onkeydown={(e) => usagesMenu && e.key === "Escape" && closeUsagesMenu()} />

<div
  class="editor"
  bind:this={host}
  role="group"
  aria-label="PseudoScript source editor"
></div>

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
    box-shadow: 0 16px 40px -16px rgba(0, 0, 0, 0.85);
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
