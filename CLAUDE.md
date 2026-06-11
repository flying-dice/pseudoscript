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

`model/` is a large worked-example model — PseudoScript modeling its own design: one `.pds` module per compiler crate (bounded context), plus the web IDE (`ide`, the single typed `pseudoscript-ide` wasm) and the landing site (`landing`). A buildable workspace (`pds doc model`). By convention a project's PseudoScript model lives at `<root>/model`. The flagship worked examples live under `web-ide/src/lib/samples/` (one folder per sample, each a buildable `pds` workspace plus a `meta.json` for the IDE's examples picker).

When changing a language rule, the change usually touches several of these at once: the `LANG.md` clause, a `decisions/` entry if a fork was resolved, and a `CONFORMANCE/` case that exercises it. The ADRs each list the `LANG.md` sections they affect.

### Where `.pds` examples live — keep every one valid under the current rules

A rule change (FQN form, visibility, syntax) must leave every worked example compiling and exemplary. The example surfaces, beyond the four artifacts above:

- **`.claude/skills/pseudocode/SKILL.md`** — the authoring/mapping skill, with a worked model. The IDE's "Download skill" button serves it from **`web-ide/static/pseudocode-skill.zip`**, rebuilt by `npm run bundle:skill` (in `web-ide`); the zip also vendors `references/LANG.md`. Re-bundle and commit the zip whenever the skill or `LANG.md` changes.
- **`web-ide/src/lib/workspace.ts`** — `starterModule` / `emptySeed`, the `.pds` a brand-new empty project scaffolds. It is the first model every user sees, so it MUST compile clean.
- **`model/`** and **`web-ide/src/lib/samples/<name>/`** — buildable example workspaces (each has a `pds.toml`).
- The worked example in **`LANG.md`** and the snippets in **`PATTERNS.md`**.

Validate an example as a **workspace**: `pds doc <dir>` resolves each file to its module FQN and applies the full-qualification checks (a reference is its flat FQN `module::Name`, §8.1). The doc site is written to `<dir>/target/doc` — except the self-model, whose `[doc].out` is `site/`: `model/site` is **tracked and committed** (deployed to Cloudflare Pages by `deploy-model-docs.yml`), so rebuild it with `pds doc model` and commit it alongside any model change.

## Model-driven engineering — model first, then impl (mandatory)

The self-model `model/` is the **spec for the toolchain implementation**: changing the behaviour or architecture of any crate MUST start in the model, then the code is aligned to it — never the reverse. (This is distinct from `LANG.md`/`CONFORMANCE/` leading the *language* definition; MDE governs the `crates/` implementation against its self-model.)

The order for any behavioural change:

1. **Amend the model.** Edit `model/<module>.pds` so the disclosed callable bodies state the new business logic — every guard, every `Err`/`Missing`/variant arm, the order of operations, the dependency calls, the `from` provenance. Keep it C4-level: disclose decisions and data-flow, black-box plumbing.
2. **`pds doc model` clean.** The self-model MUST resolve with zero diagnostics before any code changes.
3. **Align the implementation.** Translate the disclosed bodies into the crate(s) — preserve every error arm and the order of operations; the disclosed body is normative. Re-add black boxes as adapters and omitted plumbing as glue, but invent no business logic the model doesn't show.
4. **Keep the guard green.** `crates/pseudoscript/tests/model_conformance.rs` enforces model↔code correspondence: no cross-module call may land on a `component` (publish the contract on the container/system face — see `.claude/skills/pseudocode/SKILL.md`), each crate exposes the public face the model names, and engines the model keeps private stay encapsulated. `cargo test -p pseudoscript --test model_conformance` MUST pass.

A behavioural code change that is not first reflected in `model/` is a defect: the model is the source, the code is its faithful realisation.

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

A Cargo workspace (`resolver = "3"`, edition 2024) with several members under `crates/` — `pseudoscript-syntax` (lexer/parser), `pseudoscript-model` (resolution + checks), `pseudoscript-format`, `pseudoscript-emit` (diagrams), `pseudoscript-doc`, `pseudoscript-lsp` / `pseudoscript-lsp-core`, `pseudoscript-ide` (the web IDE's single typed wasm) — plus `crates/pseudoscript`, whose binary is `pds`. Standard commands: `cargo build`, `cargo test`, `cargo test -p pseudoscript <name>` for a single test. The `pds` binary wraps the libraries: `check`/`eval` (diagnostics — `eval` reads stdin so an agent can check a snippet without a file), `fmt`, `tokens`, `doc`, `outline`, `svg`, `lsp`, `lang`/`skill`, and the `add`/`install`/`update` dependency commands. When writing Rust, the `idiomatic-rust` skill is required.

## One language core, two edges — do not fork it

Language intelligence has a single source of truth: **`crates/pseudoscript-lsp-core`** — transport-neutral `text + position -> lsp_types value` handlers (completion, hover, definition, references, semantic tokens, folding, symbols, diagnostics, formatting). It depends on `pseudoscript-model` + the standalone `lsp-types` (pinned to tower-lsp 0.20's `=0.94.1`, WASM-safe), **not** `tower-lsp`. Two edges consume it:

- **`crates/pseudoscript-lsp`** — the stdio server (`server.rs` + `workspace.rs`): tower-lsp transport over `lsp-core`. What native editors (e.g. `pseudoscript-jetbrains`) get.
- **`crates/pseudoscript-ide`** — the browser IDE's single wasm. It is the whole web-IDE application: a stateful `IdeSession` (`src/lib.rs`) that holds the workspace and exposes **one typed surface** covering every capability the IDE drives — language intelligence (routed through `lsp-core`), diagrams (`pseudoscript-emit`), the doc site (`pseudoscript-doc`), formatting, and dependency resolution. The toolchain crates are consumed as ordinary Rust rlibs, not a second wasm. The boundary is typed with **`tsify`**: the Rust DTOs are the source of truth, wasm-bindgen emits the real `.d.ts`, and values cross as objects — no hand-written TS, no `JSON.parse`. The one exception is the render IR `Scene`, an opaque JSON string the canvas reads structurally. The web IDE (`web-ide/src/lib/pds.ts`) is the only client and drives `IdeSession` directly, so the wasm toolchain is exercised as a real client and cannot drift.

To change a language feature, edit it once in `lsp-core` (or its `model` primitive). After the wasm surface changes, rebuild: `npm run build:wasm` in `web-ide`, then commit the regenerated `pds-ide-wasm/` artifacts. Highlight **colours** and pure editor UX stay client-side.

## Playwright

Always write playwright tests for web-ide and web-landing projects.
Playwright tests must use data-testid attributes for element selection. No brittle CSS selectors.
Do not guess how to navigate using playwright explore the site and use the data to build the tests.