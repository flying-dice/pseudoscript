// CodeMirror 6 language support for PseudoScript. Highlighting is AST-aware:
// it decorates the wasm compiler's semantic tokens (the same colouring the LSP
// serves), not a hand-written regex tokenizer. The StreamLanguage is retained
// only for editor structure (comment-toggle). Completion and linting delegate
// to the wasm compiler too.
import { autocompletion } from "@codemirror/autocomplete";
import type {
  Completion,
  CompletionContext,
  CompletionResult,
} from "@codemirror/autocomplete";
import { LanguageSupport, StreamLanguage } from "@codemirror/language";
import type { StringStream } from "@codemirror/language";
import { linter } from "@codemirror/lint";
import type { Diagnostic as LintDiagnostic } from "@codemirror/lint";
import { Decoration, EditorView, ViewPlugin } from "@codemirror/view";
import type { DecorationSet, ViewUpdate } from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";
import type { Extension } from "@codemirror/state";

import { check, semanticTokens } from "./pds.js";
import type { CompletionItem } from "./pds.js";
import { byteToChar } from "./offsets.js";

// The compiler diagnostic fields this linter reads. `check`'s wasm payload
// carries raw UTF-8 byte offsets (`start`/`end`) alongside the line/column form;
// the linter maps those byte offsets to CodeMirror's code-unit positions.
interface CompilerDiagnostic {
  severity: string;
  message: string;
  start: number;
  end: number;
}

// A semantic-token role: the key set of the LSP token-type legend.
type SemKind =
  | "namespace"
  | "type"
  | "class"
  | "parameter"
  | "variable"
  | "property"
  | "enumMember"
  | "method"
  | "keyword"
  | "comment"
  | "string"
  | "number"
  | "decorator";

// The shape the editor expects of the caller's completion source: it maps a
// CodeMirror completion context to the LSP completion items for the caret.
export type CompletionFetcher = (
  context: CompletionContext,
) => CompletionItem[] | null | undefined;

// A structure-only language: it carries the `//` comment config (so Mod-/ works)
// but does no colouring — the semantic-token decorator below owns highlighting,
// so the keyword/token lists no longer live in JS where they could drift from
// the compiler's lexer.
const streamLang = StreamLanguage.define({
  name: "pseudoscript",
  languageData: { commentTokens: { line: "//" } },
  token(stream: StringStream): string | null {
    stream.next();
    return null;
  },
});

// Each semantic-token role → a stable CSS class (used by the theme and by tests
// via `data` attributes on the marks). Names mirror the LSP token types.
const SEM_CLASS: Record<SemKind, string> = {
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
const SEM_DECO: Partial<Record<SemKind, Decoration>> = Object.fromEntries(
  Object.entries(SEM_CLASS).map(([kind, cls]): [SemKind, Decoration] => [
    kind as SemKind,
    Decoration.mark({ class: cls, attributes: { "data-sem": kind } }),
  ]),
);

// The token-type legend, in the index order the LSP semantic-tokens response
// uses (must match pseudoscript-lsp-core::semantic::token_types). The response's
// `token_type` field indexes this.
const SEM_LEGEND: SemKind[] = [
  "namespace", "type", "class", "parameter", "variable", "property",
  "enumMember", "method", "keyword", "comment", "string", "number", "decorator",
];

// Decorates `view` from the compiler's LSP semantic-tokens response. The result
// is the standard delta encoding (`{ data: [Δline, Δstart, len, type, mods] }`)
// over UTF-16 positions — the same units CodeMirror uses, so no byte conversion.
function semanticDecorations(view: EditorView): DecorationSet {
  const doc = view.state.doc;
  let data: number[];
  try {
    data = semanticTokens(doc.toString()).data ?? [];
  } catch {
    return Decoration.none; // transient parse failure — leave text uncoloured
  }
  const builder = new RangeSetBuilder<Decoration>();
  let line = 0;
  let char = 0;
  for (let i = 0; i + 4 < data.length; i += 5) {
    const [dLine, dStart, len, typeIdx] = data.slice(i, i + 5);
    if (dLine === 0) char += dStart;
    else {
      line += dLine;
      char = dStart;
    }
    const deco = SEM_DECO[SEM_LEGEND[typeIdx]];
    if (!deco || len === 0 || line >= doc.lines) continue;
    const from = doc.line(line + 1).from + char;
    const to = from + len;
    if (to > from && to <= doc.length) builder.add(from, to, deco);
  }
  return builder.finish();
}

// Recomputes the whole token set on every document change. The compiler parse is
// sub-millisecond for IDE-sized files; tokens are already sorted and
// non-overlapping, so the RangeSetBuilder consumes them directly.
const semanticHighlighter = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = semanticDecorations(view);
    }
    update(update: ViewUpdate): void {
      if (update.docChanged) this.decorations = semanticDecorations(update.view);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

// Colours for each role, taken from the theme's `--hl-*` tokens so they flip
// with light/dark mode (the light values are darker for contrast on paper).
const semanticTheme = EditorView.theme({
  ".pst-keyword": { color: "var(--hl-keyword)", fontWeight: "600" },
  ".pst-namespace": { color: "var(--hl-namespace)" },
  ".pst-type": { color: "var(--hl-type)" },
  ".pst-class": { color: "var(--hl-class)" },
  ".pst-parameter": { color: "var(--hl-parameter)" },
  ".pst-variable": { color: "var(--hl-variable)" },
  ".pst-property": { color: "var(--hl-property)" },
  ".pst-enumMember": { color: "var(--hl-enum)" },
  ".pst-method": { color: "var(--hl-method)" },
  ".pst-comment": {
    color: "var(--hl-comment)",
    fontStyle: "italic",
    fontFamily: "var(--font-prose, ui-sans-serif, system-ui, sans-serif)",
  },
  ".pst-string": { color: "var(--hl-string)" },
  ".pst-number": { color: "var(--hl-number)" },
  ".pst-decorator": { color: "var(--hl-decorator)" },
});

/** The PseudoScript language with AST-aware (LSP-sourced) highlighting. */
export function pseudoscript(): LanguageSupport {
  return new LanguageSupport(streamLang, [semanticHighlighter, semanticTheme]);
}

// Maps the LSP `CompletionItemKind` (an integer enum) to the CodeMirror option
// type that drives each candidate's icon.
const KIND_TYPE: Record<number, string> = {
  2: "method", // Method
  3: "function", // Function (built-in macro)
  5: "property", // Field
  7: "class", // Class (data)
  9: "namespace", // Module (node)
  14: "keyword", // Keyword
  22: "type", // Struct (primitive / type)
};

// Autocomplete sourced from the shared LSP completion engine (the same one the
// native language server serves), so the web IDE narrows by context — members
// after `.`, a module's symbols after `::`, macros after `#[`, types in type
// position — instead of always offering every keyword and symbol.
//
// `getCompletions(context)` returns `[{ label, kind, detail }]` for the caret;
// the labels are bare segment names, so completion replaces only the identifier
// segment under the caret (after the last `.`/`::`), and CodeMirror filters the
// returned set against the typed prefix.
export function pseudoscriptCompletion(
  getCompletions: CompletionFetcher,
): Extension {
  return autocompletion({
    activateOnTyping: true,
    icons: true,
    override: [
      (context: CompletionContext): CompletionResult | null => {
        // Auto-open once a prefix is typed, or right after a `.`/`::` trigger
        // (the engine offers the member / submodule set at that boundary, with no
        // prefix yet). An explicit invoke (Ctrl-Space) still completes anywhere.
        // Only the trailing identifier segment is replaced — the `.`/`::` before
        // it is context the engine already accounted for.
        const word = context.matchBefore(/[A-Za-z_]\w*/);
        const atTrigger = context.matchBefore(/\.|::/);
        if (!word && !atTrigger && !context.explicit) return null;
        const from = word ? word.from : context.pos;
        const seen = new Set<string>();
        const options: Completion[] = [];
        for (const c of getCompletions(context) ?? []) {
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
export function pseudoscriptLinter(): Extension {
  return linter((view: EditorView): LintDiagnostic[] => {
    const source = view.state.doc.toString();
    let diagnostics: CompilerDiagnostic[];
    try {
      diagnostics = check(source) as unknown as CompilerDiagnostic[];
    } catch {
      return [];
    }
    const length = view.state.doc.length;
    return diagnostics.map((d): LintDiagnostic => {
      // Compiler spans are UTF-8 byte offsets; map to code-unit offsets.
      const from = Math.min(byteToChar(source, d.start), length);
      const to = Math.min(Math.max(byteToChar(source, d.end), from), length);
      const severity: LintDiagnostic["severity"] =
        d.severity === "error" ? "error" : d.severity === "warning" ? "warning" : "info";
      return { from, to, severity, message: d.message };
    });
  });
}
