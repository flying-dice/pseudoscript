# ADR-020 — Return-type and `from` checking for determinable forms

**Status:** Accepted (superseded by ADR-035 — `from` targets any non-node type and checks a single-expression source)
**Affects:** LANG.md §5.1, §7.2

## Context

The checker did no type matching: it narrowed `Result`/`Option` accessors (§6) but never compared a `return` expression to the declared return type, and never validated a `from` target. So `one(): number { return "" }` and `Diagnostic from { … }` returned as a `Diagnostic[]` both passed clean — type errors the spec implies (§5.1 return semantics, §7.2 "a record `data` or a union variant, usable wherever a value of `Type` is expected") but the implementation never enforced.

Types remain shape hints (§1): the checker does not infer the type of a binding, call, or field access. The check applies only where a value's type is statically determinable from its syntactic form.

## Decision

- A `return` expression is type-checked when its type is statically known:
  - a string/number/bool **literal** → that primitive;
  - an `Ok`/`Err` **marker** → `Result`; a `Some`/`None` marker → `Option`;
  - a `Type from { … }` composition → `Type` (a single value).
  Its type MUST match the declared return type. A union variant satisfies its union type (§3.5). A bare binding, call, or field access is not inferred and is not checked.
- A `from` target MUST resolve to a `data` record or union variant; a primitive, `Result`, `Option`, or node target MUST be rejected (§7.2). A target this single-module checker cannot see (another module) is left alone, mirroring the `for`-parent check (ADR-010).
- `from` yields a single value; using it where an array (`T[]`) is expected at a checked return site MUST be rejected.

## Consequences

- §5.1: the return-type-match clause.
- §7.2: the `from`-target-is-a-record and single-value clauses.
- The CLI (`pds check`) and the LSP both run `model::check`, so both surface these diagnostics.
- Tested first via the `type_checking.feature` cucumber suite (red→green) and the `static/5-return-type-mismatch`, `static/7-from-not-a-data-target`, `static/7-from-singular-as-array` conformance cases.
- Scope boundary: full inference (binding/call/field-access types, call-argument types) is not done; those returns stay unchecked. A later ADR may extend coverage.
- Rejected alternative: full static typing of every expression — rejected for now as a larger reversal of the shape-hints stance (§1); this ADR enforces only the determinable forms the spec already pins.
