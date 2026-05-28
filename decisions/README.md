# Decisions

Architecture decision records for PseudoScript. Each pins a resolved fork out of `LANG.md` and into history. Format: `00N-name.md`.

## [001 — Result is the only fallible type (no Option)](001-result-only-no-option.md)

`Result<T, E>` is the only built-in generic; `Option`, `Some`, and `None` do not exist. The `?` clause here is superseded by ADR-008.

Read in full for the rationale that a static model gains no power from a second absence type, and for the full list of `LANG.md` edits (§3.2, §6, §10).

## [002 — Bindings are immutable (single-assignment)](002-immutable-bindings.md)

A name is bound exactly once; reassignment and shadowing MUST be rejected.

Read in full when implementing the rebind/shadowing checks, or for the `static/7-rebind-rejected` conformance case it enables.

## [003 — No value construction; Ok/Err are result markers](003-no-construction-result-markers.md)

PseudoScript never instantiates values. `Ok`/`Err` are result markers that tag a return branch; `data` describes shape only; `from` is the sole value-combining form and records provenance.

Read in full to see why `CtorExpr` became `ResultMarker` and how this reinforces ADR-001.

## [004 — Self/sibling calls via `self.`](004-self-calls.md)

`self` refers to the enclosing node; same-node callables are invoked as `self.Name(args)`. Bare `Name(args)` does not resolve.

Read in full for the sequence-diagram rendering (self-message on the lifeline) and the `self`-only-in-body scoping rule.

## [005 — Unhandled Result is allowed](005-dropped-result-allowed.md)

A call statement MAY drop its `Result` with no diagnostic; explicit `if` handling is the idiom, not a requirement.

Read in full when deciding whether to warn on unconsumed results — this ADR rejects that, and explains the progressive-disclosure reasoning.

## [006 — Union variants: inline declares and hoists, bare references](006-union-variants.md)

`| Name { ... }` declares a variant and hoists it to the module namespace; bare `| Name` references an existing module-level `data`. Missing references and name collisions MUST be rejected.

Read in full for the declare-vs-reference worked example and the namespace-scoping alternative that was rejected.

## [007 — Full `.` chaining](007-full-chaining.md)

`.` access and call chaining are unrestricted (`a.b.c`, `Repo.fetch(id).value`, `a.f().g()`), evaluated left-to-right.

Read in full for the left-recursive `Postfix` grammar form and the per-call sequence-diagram mapping.

## [008 — No optionality marker (`?` removed)](008-no-optionality-marker.md)

`?` is removed entirely; `[]` is the only type suffix. Amends ADR-001, which had kept `?`.

Read in full for why the optionality concept was judged too thin to keep, and the resulting `Type = Named [ "[]" ]` grammar.

## [009 — Doc-comment summary/body split on a blank `///` line](009-doc-paragraph-delimiter.md)

A `///` line with no text ends the summary; everything before it is the summary, everything after is the extended description.

Read in full for the delimiter example and the first-line/first-sentence alternatives that were rejected.

## [010 — `for` parent: FQN addressing and kind rules](010-for-parent.md)

A `for` parent is a `Path` resolved as an FQN; a cross-module parent MUST be `public`. A container's parent MUST be a `system`, a component's MUST be a `container`.

Read in full when implementing parent resolution or the kind checks, and for the cross-module parenting example.

## [011 — Block contents: person behavior, callables-only blocks](011-block-contents.md)

A `person` MAY own callables (overturning the earlier no-behavior rule). A disclosed block holds callables only; containers and components never nest inside a block.

Read in full for the impact on sequence diagrams (persons as participants) and the spread of edits across §1, §4, §5, §9, §10.

## [012 — Reserved words and case sensitivity](012-reserved-words-case.md)

Keywords, primitive type names, `Result`, `Ok`, and `Err` are reserved. Identifiers are case-sensitive; the PascalCase/lowercase convention MUST NOT be enforced.

Read in full for the exact reserved set and the rejected enforced-casing alternative.

## [013 — Literal forms and placement](013-literals.md)

`Literal` is a string, number, or bool; `true`/`false` are reserved. Literals MAY appear as macro and call arguments. Extends ADR-012.

Read in full for the `Literal` grammar and the string-only / macro-args-only alternatives that were rejected.

## [014 — `for` iterates arrays only](014-for-iterables.md)

`for (x in Expr)` requires `Expr` to be an array `T[]`; `x` binds to `T`. Iterating a non-array MUST be rejected.

Read in full for the binding-type rule and the rejected iterate-`Result` alternative.

## [015 — Per-macro target constraints](015-macro-targeting.md)

Each built-in macro declares which declaration kinds it may attach to; the grammar permits a macro anywhere and the checker enforces targeting. All four current macros target callables.

Read in full when adding a macro or its target check, and for why targeting is a static rule rather than a grammar rule.

## [016 — Non-void callables must return on all paths](016-return-coverage.md)

A non-`void` callable MUST return on every path; a fall-through branch MUST be rejected. A `void` callable needs no return.

Read in full for the all-paths example and the `static/5-missing-return` conformance case it enables.
