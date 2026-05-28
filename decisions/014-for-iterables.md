# ADR-014 — `for` iterates arrays only

**Status:** Accepted
**Affects:** LANG.md §7, §10

## Context

`for (x in Expr)` left the iterable type unstated. The array `T[]` is the only collection type in the language.

## Decision

- `Expr` in a `for (x in Expr)` loop MUST be an array type `T[]`.
- The loop variable `x` is bound to `T` for each iteration.
- Iterating a non-array MUST be rejected.

```pds
for (acc in accounts) {   // accounts: Account[]
  self.Process(acc)
}
```

## Consequences

- §7: states the array-only rule and the binding type of `x`.
- §10: `For` unchanged; constraint enforced by the checker.
- Rejected alternative: iterating `Result<T[], E>` directly.
