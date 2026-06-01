// CodeMirror 6 language support for PseudoScript. Highlighting is AST-aware:
// it decorates the wasm compiler's semantic tokens (the same colouring the LSP
// serves), not a hand-written regex tokenizer. The StreamLanguage is retained
// only for editor structure (comment-toggle). Completion and linting delegate
// to the wasm compiler too.
import { autocompletion } from "@codemirror/autocomplete";
import { LanguageSupport, StreamLanguage } from "@codemirror/language";
import { linter } from "@codemirror/lint";
import { Decoration, EditorView, ViewPlugin } from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";

import { check, semanticTokens } from "./pds.js";
import { byteToChar } from "./offsets.js";

// A structure-only language: it carries the `//` comment config (so Mod-/ works)
// but does no colouring — the semantic-token decorator below owns highlighting,
// so the keyword/token lists no longer live in JS where they could drift from
// the compiler's lexer.
const streamLang = StreamLanguage.define({
  name: "pseudoscript",
  languageData: { commentTokens: { line: "//" } },
  token(stream) {
    stream.next();
    return null;
  },
});

// Each semantic-token role → a stable CSS class (used by the theme and by tests
// via `data` attributes on the marks). Names mirror the LSP token types.
const SEM_CLASS = {
  namespace: "pst-namespace",
  type: "pst-type",
  class: "pst-class",
  parameter: "pst-parameter",
  variable: "pst-variable",
  property: "pst-property",
  enumMember: "pst-enumMember",
  method: "pst-method",
  keyword: "pst-keyword",
  comment: "pst-comment",
  string: "pst-string",
  number: "pst-number",
  decorator: "pst-decorator",
};

// One reusable mark decoration per role. A `data-sem` attribute carries the role
// for robust, non-brittle test selection.
const SEM_DECO = Object.fromEntries(
  Object.entries(SEM_CLASS).map(([kind, cls]) => [
    kind,
    Decoration.mark({ class: cls, attributes: { "data-sem": kind } }),
  ]),
);

/** Builds the decoration set for `view` from the compiler's semantic tokens. */
function semanticDecorations(view) {
  const src = view.state.doc.toString();
  let tokens;
  try {
    tokens = semanticTokens(src);
  } catch {
    return Decoration.none; // transient parse failure — leave text uncoloured
  }
  const builder = new RangeSetBuilder();
  for (const tok of tokens) {
    const deco = SEM_DECO[tok.kind];
    if (!deco) continue;
    const from = byteToChar(src, tok.start);
    const to = byteToChar(src, tok.end);
    if (to > from) builder.add(from, to, deco);
  }
  return builder.finish();
}

// Recomputes the whole token set on every document change. The compiler parse is
// sub-millisecond for IDE-sized files; tokens are already sorted and
// non-overlapping, so the RangeSetBuilder consumes them directly.
const semanticHighlighter = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = semanticDecorations(view);
    }
    update(update) {
      if (update.docChanged) this.decorations = semanticDecorations(update.view);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

// Colours for each role, themed off the editor's CSS variables.
const semanticTheme = EditorView.theme({
  ".pst-keyword": { color: "var(--accent)", fontWeight: "600" },
  ".pst-namespace": { color: "#2dd4bf" },
  ".pst-type": { color: "#5eead4" },
  ".pst-class": { color: "#2dd4bf" },
  ".pst-parameter": { color: "#d6a96b" },
  ".pst-variable": { color: "var(--ink)" },
  ".pst-property": { color: "#dcc98a" },
  ".pst-enumMember": { color: "#6e8bff" },
  ".pst-method": { color: "#82aaff" },
  ".pst-comment": {
    color: "var(--ink-faint)",
    fontStyle: "italic",
    fontFamily: "var(--font-prose, ui-sans-serif, system-ui, sans-serif)",
  },
  ".pst-string": { color: "#7fd88f" },
  ".pst-number": { color: "#e0a93f" },
  ".pst-decorator": { color: "#e0a93f" },
});

/** The PseudoScript language with AST-aware (LSP-sourced) highlighting. */
export function pseudoscript() {
  return new LanguageSupport(streamLang, [semanticHighlighter, semanticTheme]);
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
