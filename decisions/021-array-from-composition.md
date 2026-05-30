# ADR-021 — `from` can compose an array (`Type[] from { … }`)

**Status:** Accepted
**Amends:** ADR-020 (the "`from` yields a single value" rule)
**Affects:** LANG.md §7.2, §10

## Context

`from` composed a single record or union variant (§7.2), so a callable returning `Type[]` had no way to express its result — `Type from { … }` yields a single `Type`, which ADR-020's return-type check (correctly) rejects against a `Type[]` declaration. A common shape — "combine these into a list" (e.g. concatenating diagnostic lists) — was inexpressible, and `alias` is a node shorthand (§8.3), not a type alias, so it cannot stand in.

## Decision

- `Type[] from { … }` composes an array `Type[]`; the `[]` suffix on the target marks the result as an array. `Type from { … }` is unchanged (a single value).
- The target `Type` is still a `data` record or union variant (§7.2); `[]` only changes the result's cardinality.
- An array `from` satisfies an array return/expected type; a singular `from` does not (and vice versa) — the ADR-020 return-type check compares array-ness.
- `for (x in Type[] from { … })` iterates the composed array; `for (x in Type from { … })` is still rejected (ADR-014).

## Consequences

- §7.2: the single-value clause becomes "a single value, or `Type[] from` an array".
- §10: `FromExpr = Path [ "[]" ] "from" "{" … "}"`.
- AST: `ExprKind::From` carries `is_array`; the formatter prints the `[]`.
- The checker's return-type and `for`-iterable checks compare array-ness.
- `alias` is unchanged — it remains a node shorthand (§8.3), not a type alias.
- Rejected alternative: a general type-alias form (`alias Name = Type[]`). Out of scope — `alias` binds nodes; array composition is the actual need and is expressed directly.
