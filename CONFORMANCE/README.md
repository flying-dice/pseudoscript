# Conformance

The executable contract every PseudoScript implementation must satisfy. Where `LANG.md` describes the language in prose and notation, this directory describes it in runnable cases. An implementation that passes every case here matches the spec; one that doesn't, doesn't.

## Layout

```
CONFORMANCE/
├── lexical/      ← tokenisation cases            (LANG.md §2)
├── syntax/       ← parse / reject cases          (LANG.md §3–§10 grammar)
├── static/       ← resolution / well-formedness  (LANG.md §6, §8)
└── generation/   ← diagram generation            (LANG.md §9) — deferred
```

One sub-directory per spec layer. A test in `lexical/` exercises the lexer in isolation; one in `static/` runs the lexer, parser, and checker together.

## File conventions

| Layer | Inputs | Expected output |
| --- | --- | --- |
| `lexical/` | `name.pds` | `name.tokens` — one token per line, format `KIND@line:col "lexeme"`. See `lexical/README.md` for the token taxonomy. |
| `syntax/`  | `name.pds` (accept) | the source must parse against the §10 grammar with no error node. A second form, `name.reject`, is a source the parser must reject; `name.reject.expected` names the error category in prose. |
| `static/`  | `name.pds` | `name.diagnostics` — the set of static errors the checker must report, one per line (empty file = well-formed). The match is order-independent. |
| `generation/` | `name.pds` | **Deferred** — see `generation/README.md`. Output is custom SVG (LANG.md §1, §9); the stable assertion surface is the pre-render `Scene` IR, whose shape isn't specified yet. |

Pairs live side-by-side. The test runner glob is `*.pds` and the expected files are derived by extension.

## Naming

Each case file starts with the `LANG.md` section it exercises, then a short descriptive slug:

```
static/6-result-wrong-accessor.pds           ← the source
static/6-result-wrong-accessor.diagnostics    ← the errors the checker must report
```

`6` means `LANG.md` §6 (Errors). A `2-4` prefix means §2.4. This keeps the conformance directory navigable in alphabetical order and lets a reviewer trace from a case back to the rule it tests.

## What a passing implementation looks like

A PseudoScript implementation passes conformance when, for every `*.pds` file under `CONFORMANCE/`:

1. The implementation produces the expected tokenisation, parse result, or diagnostics (as appropriate for the layer).
2. No additional warnings or diagnostics are produced beyond those listed in the expected output.
3. The exit status is zero (success) for cases that should succeed, non-zero for cases that should fail.

The conformance files themselves are language- and implementation-agnostic. A PseudoScript implementation in any host language is expected to bring its own runner — one visible test per case per layer.

## Adding cases

When a new case is added:

- Cite the spec section it exercises in the filename prefix.
- The case must be minimal — exercise one rule, not five.
- The expected outputs must be hand-written (or hand-reviewed if generated), never copied from whatever the current implementation happens to produce. The spec leads; the impl follows.

## Known gaps

- **Cross-module visibility** (§8.2) needs multi-file fixtures: PseudoScript FQNs are file-derived (one `.pds` file = one module), so a `public`/private cross-module case cannot be expressed as a single file. The `*.pds`-per-case convention does not yet model a multi-file workspace; these cases are future work.
- **Generation** (§9) is deferred until the spec pins a diagram notation.

## What conformance does not test

- Performance characteristics. The spec defines correctness, not throughput.
- Diagnostic wording. Two implementations may format the same error differently; conformance pins the error *kind* via `name.diagnostics`, not the exact prose.
- Diagram layout / styling. The spec defines what nodes and edges a model yields, not pixel placement.
