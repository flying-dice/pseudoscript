# ADR-019 — Option reinstated; the built-in generics are constructed by their markers

**Status:** Accepted
**Supersedes:** ADR-001 (the no-`Option` rule), ADR-003 (the no-construction rule, for the built-in generics)
**Affects:** LANG.md §2.3, §3.2, §3.3, §6, §7.2, §10

## Context

ADR-001 made `Result<T, E>` the only built-in generic and barred `Option`/`Some`/`None`; ADR-003 barred value construction, casting `Ok`/`Err` as branch markers that build nothing. Modeling an optional value then forced a `Result<T, _>` stand-in or a prose note. Reinstating `Option`, with `Some`/`None` constructing it exactly as `Ok`/`Err` construct `Result`, restores the absent-value vocabulary.

## Decision

- `Option<T>` is a built-in generic alongside `Result<T, E>`.
- `Some` and `None` are reserved keywords that construct an `Option`: `Some(v)` wraps a `T`, `None` is the empty case — parallel to `Ok(v)` / `Err(e)` for `Result`.
- The built-in generics ARE constructed by these four markers. `data` records and union variants are still produced only by `from` (§7.2); no other construction exists.
- Option accessors: `o.isSome`, `o.isNone`, `o.value` (the `T`). `Option` has no `.error`. Accessing `.value` on a `None` is a model error; the checker narrows on `if (o.isNone)` / `if (o.isSome)`, mirroring `Result` (§6).
- `Option` joins `Result` as a reserved type name.
- No `?` marker: ADR-008 stands. `Option<T>` is the only absence form; `[]` is the only type suffix.

## Consequences

- §2.3: `Some` / `None` added to the keyword list; `Option` reserved.
- §3.2: `Option<T>` listed as a built-in generic.
- §3.3: an absent value is modeled with `Option<T>`.
- §6: split into Result (§6.1) and Option (§6.2); the four markers now construct.
- §7.2: `from` composes `data`; the markers construct the generics.
- §10: `ResultMarker` → `Marker`, covering `Ok` / `Err` / `Some` / `None`.
- ADR-008 (no `?`) is unaffected.
- Rejected alternatives:
  - **A combinator method surface** (`.map`, `.andThen`, `.unwrapOr`, `.unwrap`, `.filter`, …). `Option` and `Result` expose accessors only — `isSome`/`isNone`/`value` and `isOk`/`isErr`/`value`/`error` — keeping bodies at the flow level (§1), not computation.
  - **`Some`/`None` as branch markers that construct nothing** (the ADR-003 treatment of `Ok`/`Err`). Rejected in favor of uniform construction: all four markers build a value of their generic.
