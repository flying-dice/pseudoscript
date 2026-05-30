# ADR-001 — Result is the only fallible type (no Option)

**Status:** Accepted (the `?` clause is superseded by ADR-008; the no-`Option` rule is superseded by ADR-019)
**Affects:** LANG.md §3.2, §3.3, §6, §10, §12

## Context

§3.2 declared both `Result<T, E>` and `Option<T>`, with `T?` documented as sugar for `Option<T>`. PseudoScript is static architecture pseudocode — it never instantiates or inspects runtime values, so a second fallible/absence type earns no modeling power that `Result` plus a bare optionality marker don't already give.

## Decision

- `Result<T, E>` is the only built-in generic.
- `Option`, `Some`, and `None` do not exist.
- `?` is a standalone **optionality marker** (the field or value may be absent). It carries no generic type and is not sugar for anything. *(Superseded by ADR-008: `?` is removed entirely.)*

## Consequences

- §3.2 lists `Result<T, E>` alone.
- §3.3 defines `?` directly as optionality.
- §6 drops the `Option` bullet; result accessors (`isOk`/`isErr`/`.value`/`.error`) are unaffected.
- §10 `CtorExpr` drops `Some`/`None`.
- The "Option accessors" question is void.
