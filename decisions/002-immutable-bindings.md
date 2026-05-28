# ADR-002 — Bindings are immutable (single-assignment)

**Status:** Accepted
**Affects:** LANG.md §7, §7.1, §10

## Context

§7.1 said the first assignment declares a name and later assignments reassign. PseudoScript bindings are `const`: a name in a body labels the result of one call or composition, which is then read. Reassignment has no place in a static model.

## Decision

- A name is bound exactly once. `x = Expr` introduces `x`.
- A second assignment to an already-bound name MUST be rejected.
- No shadowing: a name bound in a callable body MUST NOT be re-bound by an inner `if`/`for`/`while` block.

## Consequences

- §7.1 drops "later assignments reassign"; states single binding.
- §7 table Assignment row: "binds the name (single-assignment)".
- §10 `Assign` comment: "binds once".
- Enables conformance case `static/7-rebind-rejected`.
