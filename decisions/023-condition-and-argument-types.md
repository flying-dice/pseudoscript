# ADR-023 — Boolean conditions and call-argument types

**Status:** Accepted
**Extends:** ADR-022 (inference-based body checks)
**Affects:** LANG.md §5.1, §7

## Context

Two inference-based gaps remained after ADR-022: an `if`/`while` condition could be any type, and a call's arguments were checked only for count, not type. Both are implied — control flow tests a boolean (§7); a call binds each argument to a typed parameter (§5.1) — but were unenforced.

## Decision

Both reuse the conservative `infer` (concrete only for literals/markers/`from`/param-or-binding refs; else `Unknown`, which is never flagged).

- **Boolean conditions (§7).** An `if`/`while` condition whose inferred type is concrete MUST be `bool`. A `Result`/`Option`, an array, or a non-`bool` primitive/`data` is rejected. Accessor and call conditions (`r.isErr`, `self.ready()`) infer to `Unknown` and are not checked.
- **Argument types (§5.1).** For a call to a resolvable same-module callable, each inferable argument MUST match its parameter's type, compared by leaf name (qualified and bare forms normalize to the same leaf) with a union variant satisfying its union. Generic parameters (`Result<…>`/`Option<…>`) and `Unknown` arguments are skipped; the array flag must match.

## Consequences

- §7: an `if`/`while` condition MUST be `bool` (where inferable).
- §5.1: the arity clause gains argument-type matching.
- Cucumber: `conditions` and `argument_types` features; conformance `static/7-condition-not-bool` and `static/5-1-arg-type-mismatch`.
- Scope boundary (unchanged from ADR-022): cross-module callees, chained-receiver inference, and argument types whose inference is `Unknown` are not checked.
- Rejected alternative: comparison/boolean operators in conditions (`==`, `&&`) — still open (§12 #3); this ADR only types the existing condition forms.
