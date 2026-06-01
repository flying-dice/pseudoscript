// CodeMirror 6 language support for PseudoScript: a stream tokenizer for
// highlighting plus a linter that delegates to the wasm compiler's `check`.
import { autocompletion } from "@codemirror/autocomplete";
import { HighlightStyle, LanguageSupport, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { linter } from "@codemirror/lint";
import { tags as t } from "@lezer/highlight";

import { check } from "./pds.js";
import { byteToChar } from "./offsets.js";

const KEYWORDS = new Set([
  "system", "container", "component", "person", "data", "feature", "for",
  "public", "return", "if", "else", "while", "alias", "from", "void",
]);
const STEP_KEYWORDS = new Set(["given", "when", "then", "and", "but"]);
const ATOMS = new Set(["Result", "Option", "Ok", "Err", "Some", "None", "true", "false"]);
const PRIMITIVES = new Set(["number", "string", "bool"]);

/** Token names this mode emits, mapped to highlight tags. */
const tokenTable = {
  keyword: t.keyword,
  step: t.controlKeyword,
  comment: t.lineComment,
  doc: t.docComment,
  string: t.string,
  number: t.number,
  atom: t.atom,
  typeName: t.typeName,
  primitive: t.standard(t.typeName),
  member: t.propertyName,
  variableName: t.variableName,
  macro: t.meta,
  tag: t.tagName,
};

const streamLang = StreamLanguage.define({
  name: "pseudoscript",
  tokenTable,
  // `//` line comments — drives the toggle-comment command (Mod-/).
  languageData: { commentTokens: { line: "//" } },
  token(stream) {
    if (stream.eatSpace()) return null;
    // `//!` module doc, `///` item doc, `//` comment.
    if (stream.match(/^\/\/[/!].*/)) return "doc";
    if (stream.match(/^\/\/.*/)) return "comment";
    if (stream.match(/^"(?:[^"\\]|\\.)*"?/)) return "string";
    if (stream.match(/^#\[[^\]]*\]/)) return "macro"; // #[manual] etc.
    if (stream.match(/^#[A-Za-z0-9_-]+/)) return "tag"; // #external etc.
    if (stream.match(/^[0-9]+(?:\.[0-9]+)?/)) return "number";
    if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*/)) {
      const word = stream.current();
      // An identifier right after a `.` is a member access (a method or field) —
      // coloured distinctly from the receiving type, like Class vs Method.
      if (stream.start > 0 && stream.string[stream.start - 1] === ".") return "member";
      if (KEYWORDS.has(word)) return "keyword";
      if (STEP_KEYWORDS.has(word)) return "step";
      if (ATOMS.has(word)) return "atom";
      if (PRIMITIVES.has(word)) return "primitive";
      // An identifier directly before `(` is a callable — its definition
      // (`Summary(): …`) or a call (`Summary()`) — coloured as a member too.
      if (stream.peek() === "(") return "member";
      if (/^[A-Z]/.test(word)) return "typeName";
      return "variableName";
    }
    stream.next();
    return null;
  },
});

const highlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: "var(--accent)", fontWeight: "600" },
  { tag: t.controlKeyword, color: "#9a7bff", fontWeight: "600" },
  {
    tag: [t.lineComment],
    color: "var(--ink-faint)",
    fontStyle: "italic",
    fontFamily: "var(--font-prose, ui-sans-serif, system-ui, sans-serif)",
  },
  {
    tag: t.docComment,
    color: "#5a736c",
    fontStyle: "italic",
    fontFamily: "var(--font-prose, ui-sans-serif, system-ui, sans-serif)",
  },
  { tag: t.string, color: "#7fd88f" },
  { tag: t.number, color: "#e0a93f" },
  { tag: t.atom, color: "#6e8bff" },
  { tag: t.typeName, color: "#2dd4bf" },
  { tag: t.standard(t.typeName), color: "#b87bf5" },
  { tag: t.propertyName, color: "#dcc98a" },
  { tag: t.variableName, color: "var(--ink)" },
  { tag: t.meta, color: "#e0a93f" },
  { tag: t.tagName, color: "var(--accent-hi)" },
]);

/** The PseudoScript language + highlight styling. */
export function pseudoscript() {
  return new LanguageSupport(streamLang, [syntaxHighlighting(highlightStyle)]);
}

// Maps the LSP engine's neutral completion kind to the CodeMirror option type
// that drives each candidate's icon.
const KIND_TYPE = {
  method: "method",
  field: "property",
  keyword: "keyword",
  macro: "function",
  type: "type",
  class: "class",
  module: "namespace",
  reference: "variable",
};

/**
 * Autocomplete sourced from the shared LSP completion engine (the same one the
 * native language server serves), so the web IDE narrows by context — members
 * after `.`, a module's symbols after `::`, macros after `#[`, types in type
 * position — instead of always offering every keyword and symbol.
 *
 * `getCompletions(context)` returns `[{ label, kind, detail }]` for the caret;
 * the labels are bare segment names, so completion replaces only the identifier
 * segment under the caret (after the last `.`/`::`), and CodeMirror filters the
 * returned set against the typed prefix.
 */
export function pseudoscriptCompletion(getCompletions) {
  return autocompletion({
    activateOnTyping: true,
    icons: true,
    override: [
      (context) => {
        // Auto-open only once a prefix is typed; an explicit invoke (Ctrl-Space)
        // still completes at the bare caret. Only the trailing identifier
        // segment is replaced — the `.`/`::` before it is context the engine
        // already accounted for.
        const word = context.matchBefore(/[A-Za-z_]\w*/);
        if (!word && !context.explicit) return null;
        const from = word ? word.from : context.pos;
        const seen = new Set();
        const options = [];
        for (const c of getCompletions?.(context) ?? []) {
          if (seen.has(c.label)) continue;
          seen.add(c.label);
          options.push({ label: c.label, type: KIND_TYPE[c.kind] ?? "variable", detail: c.detail });
        }
        if (options.length === 0) return null;
        return { from, options, validFor: /^\w*$/ };
      },
    ],
  });
}

/** A linter that surfaces the wasm compiler's diagnostics inline. */
export function pseudoscriptLinter() {
  return linter((view) => {
    const source = view.state.doc.toString();
    let diagnostics;
    try {
      diagnostics = check(source);
    } catch {
      return [];
    }
    const length = view.state.doc.length;
    return diagnostics.map((d) => {
      // Compiler spans are UTF-8 byte offsets; map to code-unit offsets.
      const from = Math.min(byteToChar(source, d.start), length);
      const to = Math.min(Math.max(byteToChar(source, d.end), from), length);
      const severity = d.severity === "error" ? "error" : d.severity === "warning" ? "warning" : "info";
      return { from, to, severity, message: d.message };
    });
  });
}
