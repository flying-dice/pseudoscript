# ADR-016 — Non-void callables must return on all paths

**Status:** Accepted
**Affects:** LANG.md §5.1, §6

## Context

A disclosed callable with a return type may branch. The spec never said whether every path must end in a `return`.

## Decision

- A disclosed callable with a non-`void` return type MUST return a value on every path; a branch that falls through without returning MUST be rejected.
- A `void` callable (no return type) needs no return.

```pds
Get(id): Result<T, E> {
  if (r.isErr) { return Err(r.error) }
  return Ok(r.value)        // both paths covered
}
```

## Consequences

- §5.1: states the all-paths-return rule.
- Enables conformance case `static/5-missing-return`.
- Rejected alternative: no return-coverage check.
