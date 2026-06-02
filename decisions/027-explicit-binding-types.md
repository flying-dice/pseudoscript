# ADR-027 — Bindings state their type

**Status:** Accepted
**Amends:** ADR-002 (immutable bindings), ADR-022 (inference-based checks)
**Affects:** LANG.md §7, §7.1, §7.2, §10

## Context

A binding was untyped: `x = Expr` introduced `x` and its type was inferred from the right-hand side. Inference is best-effort (ADR-022) — a call, field access, `self`, or `::` path yields `Unknown` — so a binding fed by a call carried no known type, and a reader had to trace the callee to learn what `x` holds. The model reads as the source of truth; a name's type belongs in the source, not in an editor's inlay.

## Decision

A binding states its type: `x: Type = Expr`.

- An unannotated `x = Expr` MUST be rejected.
- Where the initialiser's type is determinable — a literal, a `from`, an `Ok`/`Err`/`Some`/`None` marker, or a bare reference — it MUST match the annotation. A call, field access, `self`, or `::` path is not inferred (ADR-022); there the annotation is authoritative and nothing is compared.
- The rule is uniform. A composition still names its type in the `from` (`x: Order = Order from { … }`); the annotation repeats it and MUST agree.

## Consequences

- §7 table Assignment / Composition rows and §7.2 examples use `x: Type = Expr`.
- §10 `Assign = Ident ":" Type "=" Expr ;`.
- The binding's type comes from the annotation, not the initialiser: hover and member completion read the declared type. Inference survives only where no annotation exists — a `for` binding's element type — and to validate an annotation against a determinable initialiser.
- ADR-002 stands: a binding is still introduced once. The introduction now carries a type.
- Conformance: `syntax/7-statements` and `syntax/7-chaining` carry annotations; a reject case pins the unannotated form.
- Rejected alternative: exempt self-typed initialisers (compositions, literals, markers) from the annotation. Two grammars for one statement, and the exemption tracks inferability — the rule a reader must hold becomes "annotate unless the right-hand side already says it." Uniform annotation is one rule.
- Rejected alternative: keep inference, surface the type as an inlay hint. The type then lives in the tool, not the model; a diff, a review, or a plain-text read loses it.
