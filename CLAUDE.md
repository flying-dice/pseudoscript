# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

This is the specification for **PseudoScript** (file extension `.pds`), an architecture-modeling language where the model *is* the source: it reads like high-level pseudocode, expresses C4-style structure (system / container / component / person), and compiles to SVG diagrams. The **spec and its conformance suite lead**: `LANG.md` and the cases under `CONFORMANCE/` are the source of truth, and expected outputs are hand-written, never copied from the implementation. The toolchain is now implemented — a Cargo workspace under `crates/` (lexer/parser, model/checker, formatter, diagram emitter, doc-site generator, LSP) driven by the `pds` binary — but it follows the spec, not the other way round. Much of the work here is still writing and refining spec prose, conformance cases, and decision records.

## Source of truth and its layers

The language is defined across four artifacts that must stay consistent with each other:

- **`LANG.md`** — the normative spec. Sections are numbered §1–§12; §10 is the EBNF grammar sketch, §12 is open questions. Everything else cites these section numbers.
- **`decisions/`** — Architecture Decision Records (`00N-name.md`). Each ADR pins one resolved fork that was pulled *out* of `LANG.md` and into history (a rejected feature, a settled evaluation-order choice). `decisions/README.md` indexes them with summaries and is **auto-loaded into context at session start** via a `SessionStart` hook in `.claude/settings.json`. Before changing any rule an ADR pins, read that ADR in full. Rejected-alternative reasoning lives here, never in `LANG.md`.
- **`CONFORMANCE/`** — the executable contract, one sub-directory per spec layer: `lexical/` (§2 tokenisation), `syntax/` (§3–§10 parse/reject), `static/` (§6, §8 resolution/well-formedness), `generation/` (§9 diagrams — **deferred**, see its README). The spec leads; expected outputs are hand-written, never copied from an implementation. Read `CONFORMANCE/README.md` before touching cases.
- **`PATTERNS.md`** — idioms and recipes for writing PseudoScript models.

`pseudoscript/` is a large worked-example model — the compiler modeling its own design, one `.pds` module per crate (bounded context), a buildable workspace (`pds doc <dir>`). The flagship worked examples live under `web-ide/src/lib/samples/` (one folder per sample, each a buildable `pds` workspace plus a `meta.json` for the IDE's examples picker).

When changing a language rule, the change usually touches several of these at once: the `LANG.md` clause, a `decisions/` entry if a fork was resolved, and a `CONFORMANCE/` case that exercises it. The ADRs each list the `LANG.md` sections they affect.

## Conformance case conventions

- Filenames start with the `LANG.md` section they exercise, then a slug: `static/6-result-wrong-accessor.pds`. `2-4` means §2.4.
- Pairs live side-by-side, derived by extension: `lexical/` → `.tokens`, `static/` → `.diagnostics` (empty file = well-formed, match is order-independent), `syntax/` accept cases are `.pds`, reject cases are `name.reject` + `name.reject.expected` (error category in prose).
- A case must be minimal — exercise one rule, not five.
- The token taxonomy (`KIND@line:col "lexeme"`) is defined in `CONFORMANCE/lexical/README.md`, not in `LANG.md`.

## Writing style — mandatory

Always invoke the `spec-style` skill before creating or editing `LANG.md`, anything under `CONFORMANCE/`, `PATTERNS.md`, or `decisions/` entries — this is mandatory, not conditional on the skill's own trigger heuristics. It enforces a terse, high-signal voice: cut hedges/ceremony/flourish, present tense only, RFC-2119 keywords (MUST/SHOULD/MAY) for normative force, and the pinned cross-reference format (`LANG.md §N` across documents, bare `§N` within `LANG.md`). Rejected-feature negations belong in `decisions/`, not the spec.

## Committing — pre-commit gate

A `PreToolUse` hook in `.claude/settings.json` **denies any `git commit`** until the `pre-commit` skill's quality gate has run (`change-review`, then `clean-code-review`, fixing every clean-code TODO marker scored > 0.5, plus lint/typecheck/tests). After the gate passes, re-issue the commit with ` # pre-commit-ok` appended to the command — that token is the hook's bypass signal. Only append it once the gate is genuinely clean.

## Rust crate

A Cargo workspace (`resolver = "3"`, edition 2024) with several members under `crates/` — `pseudoscript-syntax` (lexer/parser), `pseudoscript-model` (resolution + checks), `pseudoscript-format`, `pseudoscript-emit` (diagrams), `pseudoscript-doc`, `pseudoscript-lsp` / `pseudoscript-lsp-core`, `pseudoscript-wasm` — plus `crates/pseudoscript`, whose binary is `pds`. Standard commands: `cargo build`, `cargo test`, `cargo test -p pseudoscript <name>` for a single test. The `pds` binary wraps the libraries: `check`/`eval` (diagnostics — `eval` reads stdin so an agent can check a snippet without a file), `fmt`, `tokens`, `doc`, `outline`, `svg`, `lsp`, `lang`/`skill`, and the `add`/`install`/`update` dependency commands. When writing Rust, the `idiomatic-rust` skill is required.

## One LSP API, two transports — do not fork it

Language intelligence has a single source of truth: **`crates/pseudoscript-lsp-core`** — transport-neutral `text + position -> lsp_types value` handlers (completion, hover, definition, references, semantic tokens, folding, symbols, diagnostics, formatting). It depends on `pseudoscript-model` + the standalone `lsp-types` (pinned to tower-lsp 0.20's `=0.94.1`, WASM-safe), **not** `tower-lsp`. Two thin edges share it:

- **`crates/pseudoscript-lsp`** — the stdio server (`server.rs` + `workspace.rs`): tower-lsp transport over `lsp-core`. What native editors (e.g. `pseudoscript-jetbrains`) get.
- **`crates/pseudoscript-wasm`** — the browser bridge: each language export calls the same `lsp-core` handler and serialises its `lsp_types` result to JSON, so the WASM API is byte-for-byte the LSP API. The web IDE (`web-ide/src/lib/pds.js` + `pseudoscript-language.js`) is an LSP client — it decodes delta-encoded semantic tokens, integer `CompletionItemKind`, line-based `FoldingRange`, Markdown `Hover`. `definition`/`references` stay WASM-ergonomic (fqn-based, with previews) because LSP `Location` is URL-centric and the browser has no file URLs; they still route through `model::resolve`, so no logic forks. Diagram/doc exports (`emit_scene`, `symbol_scene`, …) are WASM-only — no LSP equivalent.

To change a language feature, edit it once in `lsp-core` (or its `model` primitive). After a *wasm-exported* surface changes, rebuild: `npm run build:wasm` in `web-ide`, then commit the regenerated `pds-wasm/` artifacts. Highlight **colours** and pure editor UX stay client-side.
