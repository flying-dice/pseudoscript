// Obsidian-style "live preview" for Markdown in CodeMirror 6.
//
// The document stays plain Markdown text (so edits round-trip and the file
// saves verbatim), but a decoration plugin renders it as you type: headings,
// bold/italic/strikethrough, inline code, blockquotes, links and rules display
// formatted, and their syntax markers (`#`, `**`, `` ` ``, `>`, `[]()`, `---`)
// hide — revealing only on the line/span the cursor is in, so you can edit them.
//
// This is the same approach Obsidian's editor uses (CM6 + the Lezer Markdown
// tree), kept to one editor engine alongside the PseudoScript `.pds` view.

import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { GFM } from "@lezer/markdown";
import { HighlightStyle, syntaxHighlighting, syntaxTree } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { StateField } from "@codemirror/state";
import type { EditorState, Extension, Range } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType } from "@codemirror/view";
import type { DecorationSet, ViewUpdate } from "@codemirror/view";
import type { SyntaxNodeRef } from "@lezer/common";

// A hidden syntax marker (`#`, `**`, brackets, …): replaced by nothing.
const hide = Decoration.replace({});

// A fenced-code delimiter line (``` or ~~~, optionally indented, with a lang).
const FENCE = /^\s*(?:`{3,}|~{3,})/;

// Highlight style for the code *inside* fenced blocks. The Markdown mode carries
// no highlight style of its own (PseudoScript's lives in its LanguageSupport),
// so the nested language tokens need one. Programming tags only — prose Markdown
// is rendered by the decorations above, not coloured here. Themed to the IDE.
const codeHighlightStyle = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.operatorKeyword, t.definitionKeyword, t.moduleKeyword], color: "var(--k-person)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--k-container)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--k-component)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--ink-faint)", fontStyle: "italic" },
  { tag: [t.typeName, t.className, t.namespace, t.changed], color: "var(--k-data)" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: "var(--k-callable)" },
  { tag: [t.variableName, t.definition(t.variableName), t.propertyName], color: "var(--ink)" },
  { tag: [t.operator, t.punctuation, t.bracket, t.separator, t.derefOperator], color: "var(--ink-soft)" },
  { tag: [t.tagName, t.angleBracket], color: "var(--k-system)" },
  { tag: [t.attributeName], color: "var(--k-component)" },
  { tag: [t.attributeValue], color: "var(--k-container)" },
  { tag: [t.invalid], color: "var(--k-system)" },
]);

// Exact pixel width of a list item's prefix (`- `, `  1. `, …) in the editor's
// prose font, so a wrapped item hangs precisely under its text. Measured with a
// canvas rather than approximated in `ch` (the proportional font's marker is
// narrower than a `0`, which overshot). Cached per font+prefix.
let measureCtx: CanvasRenderingContext2D | null = null;
const widthCache = new Map<string, number>();
function prefixWidthPx(font: string, prefix: string): number {
  const key = `${font}|${prefix}`;
  let w = widthCache.get(key);
  if (w === undefined) {
    if (!measureCtx) measureCtx = document.createElement("canvas").getContext("2d")!;
    measureCtx.font = font;
    w = measureCtx.measureText(prefix).width;
    widthCache.set(key, w);
  }
  return w;
}

// Inline spans styled in place; their markers are hidden separately.
const INLINE: Record<string, Decoration | undefined> = {
  StrongEmphasis: Decoration.mark({ class: "cm-md-strong" }),
  Emphasis: Decoration.mark({ class: "cm-md-em" }),
  Strikethrough: Decoration.mark({ class: "cm-md-strike" }),
  InlineCode: Decoration.mark({ class: "cm-md-code" }),
  Link: Decoration.mark({ class: "cm-md-link" }),
};

// Syntax-marker node names whose marker hides unless the cursor enters the span.
const INLINE_MARKS = new Set(["EmphasisMark", "StrikethroughMark", "CodeMark", "LinkMark", "URL"]);

// A rendered horizontal rule, shown in place of `---`/`***` when off the line.
class RuleWidget extends WidgetType {
  toDOM(): HTMLElement {
    const hr = document.createElement("hr");
    hr.className = "cm-md-hr";
    return hr;
  }
  ignoreEvent(): boolean {
    return false;
  }
}

// A bullet glyph rendered in place of a `-`/`*`/`+` list marker.
class BulletWidget extends WidgetType {
  eq(): boolean {
    return true;
  }
  toDOM(): HTMLElement {
    const dot = document.createElement("span");
    dot.className = "cm-md-bullet";
    dot.textContent = "•";
    return dot;
  }
  ignoreEvent(): boolean {
    return false;
  }
}

// A checkbox rendered in place of a `- [ ]` / `- [x]` task prefix. Toggling it
// rewrites the marker in the document.
class TaskWidget extends WidgetType {
  readonly checked: boolean;
  readonly from: number;
  readonly to: number;
  constructor(checked: boolean, from: number, to: number) {
    super();
    this.checked = checked;
    this.from = from;
    this.to = to;
  }
  eq(o: TaskWidget): boolean {
    return o.checked === this.checked && o.from === this.from && o.to === this.to;
  }
  toDOM(view: EditorView): HTMLElement {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-md-task";
    box.checked = this.checked;
    box.addEventListener("mousedown", (e: MouseEvent) => e.preventDefault());
    box.addEventListener("change", () => {
      view.dispatch({ changes: { from: this.from, to: this.to, insert: this.checked ? "[ ]" : "[x]" } });
    });
    return box;
  }
  ignoreEvent(): boolean {
    return true;
  }
}

// A GFM table rendered from its source text, shown in place of the pipe rows
// when the cursor is outside the table. Clicking it (off a link) drops the
// cursor in to edit the source.
class TableWidget extends WidgetType {
  readonly text: string;
  readonly from: number;
  constructor(text: string, from: number) {
    super();
    this.text = text;
    this.from = from;
  }
  eq(o: TableWidget): boolean {
    return o.text === this.text && o.from === this.from;
  }
  toDOM(view: EditorView): HTMLElement {
    const rows = this.text.split("\n").filter((l) => l.trim().length);
    const aligns = parseAligns(rows[1] || "");
    const table = document.createElement("table");
    table.className = "cm-md-table";

    const head = document.createElement("thead");
    head.appendChild(buildRow(rows[0], "th", aligns));
    table.appendChild(head);

    const body = document.createElement("tbody");
    for (let r = 2; r < rows.length; r += 1) body.appendChild(buildRow(rows[r], "td", aligns));
    table.appendChild(body);

    table.addEventListener("mousedown", (e: MouseEvent) => {
      const target = e.target;
      if (target instanceof Element && target.closest("a")) return; // let links open
      e.preventDefault();
      view.dispatch({ selection: { anchor: this.from }, scrollIntoView: true });
      view.focus();
    });
    return table;
  }
  ignoreEvent(): boolean {
    return false;
  }
}

// Per-column text alignment parsed from a table's delimiter row.
type ColAlign = "left" | "right" | "center" | null;

// Splits a table row on unescaped `|`, unescaping `\|` in each cell.
function splitPipes(s: string): string[] {
  return s.split(/(?<!\\)\|/).map((c) => c.replace(/\\\|/g, "|"));
}

// A table line's cells: trimmed, with the outer pipes dropped.
function cellsOf(line: string): string[] {
  let s = line.trim();
  if (s.startsWith("|")) s = s.slice(1);
  if (s.endsWith("|")) s = s.slice(0, -1);
  return splitPipes(s).map((c) => c.trim());
}

// Per-column alignment from a `|:--|:-:|--:|` delimiter row.
function parseAligns(delim: string): ColAlign[] {
  return cellsOf(delim).map((c): ColAlign => {
    const l = c.startsWith(":");
    const r = c.endsWith(":");
    return l && r ? "center" : r ? "right" : l ? "left" : null;
  });
}

// Builds a `<tr>` of `<th>`/`<td>` cells with inline Markdown rendered.
function buildRow(line: string, tag: "th" | "td", aligns: ColAlign[]): HTMLTableRowElement {
  const tr = document.createElement("tr");
  cellsOf(line).forEach((text, i) => {
    const cell = document.createElement(tag);
    const align = aligns[i];
    if (align) cell.style.textAlign = align;
    renderInline(text, cell);
    tr.appendChild(cell);
  });
  return tr;
}

// Inline Markdown for table cells, built with text nodes/elements (never
// innerHTML, so cell content can't inject markup). Handles code, bold, italic,
// strikethrough and links — enough for table cells; nesting is not expanded.
const INLINE_RE =
  /(`[^`]+`)|(\*\*[^*]+\*\*)|(__[^_]+__)|(~~[^~]+~~)|(\*[^*]+\*)|(_[^_]+_)|(\[[^\]]+\]\([^)]+\))/;
function renderInline(text: string, el: HTMLElement): void {
  let rest = text;
  while (rest) {
    const m = INLINE_RE.exec(rest);
    if (!m) {
      el.appendChild(document.createTextNode(rest));
      return;
    }
    if (m.index > 0) el.appendChild(document.createTextNode(rest.slice(0, m.index)));
    const tok = m[0];
    let node: HTMLElement;
    if (tok.startsWith("`")) {
      node = document.createElement("code");
      node.className = "cm-md-code";
      node.textContent = tok.slice(1, -1);
    } else if (tok.startsWith("**") || tok.startsWith("__")) {
      node = document.createElement("strong");
      node.className = "cm-md-strong";
      node.textContent = tok.slice(2, -2);
    } else if (tok.startsWith("~~")) {
      node = document.createElement("span");
      node.className = "cm-md-strike";
      node.textContent = tok.slice(2, -2);
    } else if (tok.startsWith("[")) {
      const lm = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(tok);
      const a = document.createElement("a");
      a.className = "cm-md-link";
      a.textContent = lm ? lm[1] : tok;
      if (lm && /^(https?:|mailto:|\/|#)/i.test(lm[2])) {
        a.href = lm[2];
        a.target = "_blank";
        a.rel = "noreferrer";
      }
      node = a;
    } else {
      node = document.createElement("em");
      node.className = "cm-md-em";
      node.textContent = tok.slice(1, -1);
    }
    el.appendChild(node);
    rest = rest.slice(m.index + tok.length);
  }
}

// GitHub-style alert/callout blockquotes: `> [!NOTE]` … `[!TIP]`, `[!IMPORTANT]`,
// `[!WARNING]`, `[!CAUTION]`. The first blockquote line carries the kind.
const CALLOUT_LABEL = {
  note: "Note",
  tip: "Tip",
  important: "Important",
  warning: "Warning",
  caution: "Caution",
} as const;
type CalloutKind = keyof typeof CALLOUT_LABEL;
const CALLOUT_RE = /^\s*>\s?\[!(\w+)\]/i;

// The callout kind a blockquote's first line declares, or `null`.
function calloutKind(lineText: string): CalloutKind | null {
  const m = CALLOUT_RE.exec(lineText);
  const kind = m && m[1].toLowerCase();
  return kind && Object.hasOwn(CALLOUT_LABEL, kind) ? (kind as CalloutKind) : null;
}

// The rendered callout title (the coloured "Note"/"Warning"/… label) shown in
// place of the `[!KIND]` marker when the cursor is off its line.
class CalloutTitleWidget extends WidgetType {
  readonly kind: CalloutKind;
  constructor(kind: CalloutKind) {
    super();
    this.kind = kind;
  }
  eq(o: CalloutTitleWidget): boolean {
    return o.kind === this.kind;
  }
  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-md-callout-title";
    span.textContent = CALLOUT_LABEL[this.kind];
    return span;
  }
  ignoreEvent(): boolean {
    return false;
  }
}

// Builds the decoration set for the visible ranges. The document is unchanged;
// only its rendering is decorated. A marker reveals when a selection range
// touches its "reveal range" (the heading line, the inline span, the rule line).
function decorate(view: EditorView): DecorationSet {
  const decos: Range<Decoration>[] = [];
  const { state } = view;
  const sel = state.selection;
  const touches = (from: number, to: number): boolean =>
    sel.ranges.some((r) => r.from <= to && r.to >= from);

  for (const visible of view.visibleRanges) {
    syntaxTree(state).iterate({
      from: visible.from,
      to: visible.to,
      enter: (node: SyntaxNodeRef): boolean | void => {
        const name = node.name;

        // ---- block headings: style the whole line, hide the leading `#`s ----
        const heading = /^ATXHeading(\d)$/.exec(name);
        if (heading) {
          const line = state.doc.lineAt(node.from);
          decos.push(Decoration.line({ class: `cm-md-h${heading[1]}` }).range(line.from));
          return;
        }
        if (name === "HeaderMark") {
          const line = state.doc.lineAt(node.from);
          if (!touches(line.from, line.to)) {
            // swallow the space after the `#`s too, so the text isn't indented
            let to = node.to;
            if (state.doc.sliceString(to, to + 1) === " ") to += 1;
            if (to > node.from) decos.push(hide.range(node.from, to));
          }
          return;
        }

        // ---- inline spans: style in place ----
        const inlineMark = INLINE[name];
        if (inlineMark) {
          if (node.from < node.to) decos.push(inlineMark.range(node.from, node.to));
          return;
        }

        // ---- inline markers: hide unless the cursor is inside the span ----
        if (INLINE_MARKS.has(name)) {
          const parent = node.node.parent;
          const from = parent?.from ?? node.from;
          const to = parent?.to ?? node.to;
          if (!touches(from, to) && node.from < node.to) decos.push(hide.range(node.from, node.to));
          return;
        }

        // ---- blockquote: style each line as a plain quote or, when the first
        //      line is `> [!KIND]`, a coloured callout with a title. ----
        if (name === "Blockquote") {
          const first = state.doc.lineAt(node.from);
          const kind = calloutKind(first.text);
          let lastNum = state.doc.lineAt(node.to).number;
          if (lastNum > first.number && state.doc.line(lastNum).from === node.to) lastNum -= 1;
          const cls = kind ? `cm-md-callout cm-callout-${kind}` : "cm-md-quote";
          for (let n = first.number; n <= lastNum; n += 1) {
            decos.push(Decoration.line({ class: cls }).range(state.doc.line(n).from));
          }
          // Replace the `[!KIND]` marker with the rendered title (off-cursor).
          if (kind && !touches(first.from, first.to)) {
            const lb = first.text.indexOf("[");
            const rb = first.text.indexOf("]");
            if (lb >= 0 && rb > lb) {
              decos.push(
                Decoration.replace({ widget: new CalloutTitleWidget(kind) }).range(first.from + lb, first.from + rb + 1),
              );
            }
          }
          return; // descend: QuoteMark hides the `>`, inline content renders
        }
        // Hide the `>` marker unless editing its line.
        if (name === "QuoteMark") {
          const line = state.doc.lineAt(node.from);
          if (!touches(line.from, line.to)) {
            let to = node.to;
            if (state.doc.sliceString(to, to + 1) === " ") to += 1;
            decos.push(hide.range(node.from, to));
          }
          return;
        }

        // ---- horizontal rule: render an <hr> unless editing the line ----
        if (name === "HorizontalRule") {
          const line = state.doc.lineAt(node.from);
          if (!touches(line.from, line.to)) {
            decos.push(
              Decoration.replace({ widget: new RuleWidget() }).range(line.from, line.to),
            );
          }
          return;
        }

        // ---- fenced code: a styled block; nested code is highlighted by the
        //      code-language parsers. The ``` fence lines (and the language
        //      label) collapse when the cursor is outside the block, and reveal
        //      for editing when it enters. ----
        if (name === "FencedCode") {
          const first = state.doc.lineAt(node.from);
          // Lezer ends the node at the close fence's end; if `node.to` sits at the
          // very start of the following line, that line is *not* part of the block
          // — clamp it so a blank trailing line isn't styled.
          let lastNum = state.doc.lineAt(node.to).number;
          if (lastNum > first.number && state.doc.line(lastNum).from === node.to) lastNum -= 1;
          const inBlock = touches(node.from, node.to);
          for (let n = first.number; n <= lastNum; n += 1) {
            const line = state.doc.line(n);
            const isFence = FENCE.test(line.text);
            const cls = isFence && !inBlock ? "cm-md-fence-hidden" : "cm-md-codeblock";
            decos.push(Decoration.line({ class: cls }).range(line.from));
          }
          return;
        }

        // ---- GFM table: the block <table> replacement lives in `tableField`
        //      (block decorations may not come from a plugin). Here we just skip
        //      the table's source when it's rendered, and descend to style its
        //      cells for editing when the cursor is inside. ----
        if (name === "Table") {
          return touches(node.from, node.to) ? undefined : false;
        }
      },
    });
  }

  // Hanging indent for list items: a wrapped item's continuation rows align
  // exactly under its text instead of dropping to the margin (and nested items
  // step in). The hang equals the measured prefix width, so it doesn't overshoot.
  // Done per line so it also covers an item whose own line soft-wraps.
  const cs = getComputedStyle(view.contentDOM);
  const font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
  const listRe = /^(\s*)(?:[-*+]|\d+[.)])\s+/;
  const taskRe = /^(\s*)([-*+])(\s+)(\[[ xX]\])/;
  const bulletRe = /^(\s*)[-*+](\s+)/;
  for (const visible of view.visibleRanges) {
    let pos = visible.from;
    while (pos <= visible.to) {
      const line = state.doc.lineAt(pos);
      const m = listRe.exec(line.text);
      if (m) {
        const w = prefixWidthPx(font, m[0]);
        decos.push(
          Decoration.line({
            attributes: { style: `padding-left:${w}px;text-indent:-${w}px` },
          }).range(line.from),
        );
        // Render the marker (bullet / checkbox) unless editing this line.
        if (!touches(line.from, line.to)) {
          const task = taskRe.exec(line.text);
          if (task) {
            const markFrom = line.from + task[1].length; // the `-`
            const boxFrom = markFrom + task[2].length + task[3].length; // the `[`
            const boxTo = boxFrom + task[4].length; // past the `]`
            const checked = /x/i.test(task[4]);
            decos.push(
              Decoration.replace({ widget: new TaskWidget(checked, boxFrom, boxTo) }).range(markFrom, boxTo),
            );
          } else if (bulletRe.test(line.text)) {
            const markFrom = line.from + m[1].length;
            decos.push(Decoration.replace({ widget: new BulletWidget() }).range(markFrom, markFrom + 1));
          }
        }
      }
      if (line.to + 1 <= pos) break;
      pos = line.to + 1;
    }
  }

  // RangeSet wants its members sorted; let Decoration.set sort for us.
  return Decoration.set(decos, true);
}

const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decorate(view);
    }
    update(u: ViewUpdate): void {
      if (u.docChanged || u.viewportChanged || u.selectionSet) {
        this.decorations = decorate(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

// Block-level decorations (the rendered tables) must come from a state field,
// not a plugin. Walks the whole document — fine for doc-sized files. A table
// shows its pipe source while the cursor is inside it, the <table> otherwise.
function buildTables(state: EditorState): DecorationSet {
  const decos: Range<Decoration>[] = [];
  const sel = state.selection;
  const touches = (from: number, to: number): boolean =>
    sel.ranges.some((r) => r.from <= to && r.to >= from);
  syntaxTree(state).iterate({
    enter: (node: SyntaxNodeRef): boolean | void => {
      if (node.name !== "Table") return;
      if (touches(node.from, node.to)) return false; // editing: show source
      const first = state.doc.lineAt(node.from);
      let lastNum = state.doc.lineAt(node.to).number;
      if (lastNum > first.number && state.doc.line(lastNum).from === node.to) lastNum -= 1;
      const lastLine = state.doc.line(lastNum);
      const text = state.doc.sliceString(first.from, lastLine.to);
      decos.push(
        Decoration.replace({ widget: new TableWidget(text, first.from), block: true }).range(
          first.from,
          lastLine.to,
        ),
      );
      return false;
    },
  });
  return Decoration.set(decos, true);
}

const tableField = StateField.define({
  create: (state) => buildTables(state),
  update: (value, tr) => (tr.docChanged || tr.selection ? buildTables(tr.state) : value),
  provide: (f) => EditorView.decorations.from(f),
});

// The visual styling for the rendered Markdown. Uses the IDE's CSS variables so
// it tracks the active theme.
const livePreviewTheme = EditorView.baseTheme({
  ".cm-md-h1": { fontSize: "1.7em", fontWeight: "700", lineHeight: "1.3" },
  ".cm-md-h2": { fontSize: "1.4em", fontWeight: "700", lineHeight: "1.3" },
  ".cm-md-h3": { fontSize: "1.2em", fontWeight: "700" },
  ".cm-md-h4": { fontSize: "1.08em", fontWeight: "700" },
  ".cm-md-h5, .cm-md-h6": { fontWeight: "700", color: "var(--ink-soft)" },
  ".cm-md-strong": { fontWeight: "800", color: "var(--ink)" },
  ".cm-md-em": { fontStyle: "italic" },
  ".cm-md-strike": { textDecoration: "line-through", color: "var(--ink-faint)" },
  ".cm-md-code": {
    fontFamily: "var(--font-mono)",
    fontSize: "0.9em",
    background: "var(--surface-2)",
    border: "1px solid var(--line)",
    borderRadius: "var(--radius-sm)",
    padding: "0.05em 0.3em",
  },
  ".cm-md-link": { color: "var(--accent)", textDecoration: "underline", cursor: "pointer" },
  ".cm-md-quote": {
    borderLeft: "3px solid var(--accent)",
    paddingLeft: "0.8em",
    color: "var(--ink-soft)",
    background: "var(--accent-soft)",
  },
  // GitHub-style callouts — colour set per kind via `--callout`.
  ".cm-callout-note": { "--callout": "var(--k-person)" },
  ".cm-callout-tip": { "--callout": "var(--k-container)" },
  ".cm-callout-important": { "--callout": "var(--k-data)" },
  ".cm-callout-warning": { "--callout": "var(--k-component)" },
  ".cm-callout-caution": { "--callout": "var(--k-system)" },
  ".cm-md-callout": {
    borderLeft: "3px solid var(--callout)",
    paddingLeft: "0.8em",
    background: "color-mix(in srgb, var(--callout) 10%, transparent)",
  },
  ".cm-md-callout-title": {
    color: "var(--callout)",
    fontWeight: "700",
    fontSize: "0.82em",
    textTransform: "uppercase",
    letterSpacing: "0.05em",
  },
  ".cm-md-codeblock": {
    fontFamily: "var(--font-mono)",
    fontSize: "0.88em",
    background: "var(--surface-2)",
    borderLeft: "2px solid var(--line-strong)",
  },
  // Collapse the ``` fence lines when the cursor is outside the block.
  ".cm-md-fence-hidden": { display: "none" },
  ".cm-md-bullet": { color: "var(--accent)" },
  ".cm-md-task": {
    accentColor: "var(--accent)",
    width: "1em",
    height: "1em",
    margin: "0 0.15em 0 0",
    verticalAlign: "-0.1em",
    cursor: "pointer",
  },
  ".cm-md-table": {
    borderCollapse: "collapse",
    margin: "0.4em 0",
    fontSize: "0.92em",
  },
  ".cm-md-table th, .cm-md-table td": {
    border: "1px solid var(--line-strong)",
    padding: "0.32em 0.7em",
    textAlign: "left",
  },
  ".cm-md-table th": { background: "var(--surface-2)", fontWeight: "700" },
  ".cm-md-table tbody tr:nth-child(even)": { background: "var(--surface-2)" },
  ".cm-md-hr": {
    display: "inline-block",
    width: "100%",
    border: "none",
    borderTop: "1px solid var(--line-strong)",
    margin: "0.4em 0",
    verticalAlign: "middle",
  },
});

// Options for the Markdown live-preview bundle. Reserved for future tuning;
// no fields are read yet.
export interface MarkdownLivePreviewOptions {}

/**
 * The Markdown live-preview bundle: GFM-aware parsing, the in-place render
 * decorations, and their theme. Drop into a CodeMirror editor's extensions.
 */
export function markdownLivePreview(opts: MarkdownLivePreviewOptions = {}): Extension[] {
  return [
    // `codeLanguages` nests real language parsers into fenced blocks; `languages`
    // lazy-loads each on first use, so the initial bundle stays small.
    markdown({ base: markdownLanguage, extensions: GFM, codeLanguages: languages }),
    syntaxHighlighting(codeHighlightStyle),
    tableField,
    livePreviewPlugin,
    livePreviewTheme,
    // Wrap long prose lines instead of scrolling horizontally. A wrapped line
    // stays one logical line, so the gutter shows a single number for it. The
    // fixed measure is set by `.cm-md-live .cm-content` in the app sheet.
    EditorView.lineWrapping,
    // Tags the editor root so the app sheet can give Markdown a readable,
    // lightly-technical font (a grotesk sans) instead of the `.pds` monospace,
    // and the wrapped measure — scoped to this mode, removed on swap back.
    EditorView.editorAttributes.of({ class: "cm-md-live" }),
  ];
}
