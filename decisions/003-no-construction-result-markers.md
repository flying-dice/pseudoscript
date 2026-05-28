# ADR-003 — No value construction; Ok/Err are result markers

**Status:** Accepted
**Affects:** LANG.md §2.3, §5.1, §6, §7.2, §10

## Context

§10 framed `Ok`/`Err` as `CtorExpr` ("constructors"). PseudoScript is static: it models flow and provenance, never runtime values. No `data` is ever instantiated with field values, and `Ok(x)`/`Err(e)` do not build objects — they label which branch of a `Result<T, E>` a return takes.

## Decision

- PseudoScript has no value construction.
- `Ok` and `Err` are **result markers** — reserved keywords that tag a return as the success or error branch of a `Result<T, E>`.
- `data` declarations describe shape only.
- `from` (§7.2) is the sole value-combining form, and it records provenance, not instantiation.

## Consequences

- §10: `CtorExpr` → `ResultMarker`, carrying `Ok` / `Err` only.
- §2.3: `Ok` and `Err` added to the keyword list.
- §5.1 / §6: no "construct" wording.
- Reinforces ADR-001 (no `Some`/`None`).
