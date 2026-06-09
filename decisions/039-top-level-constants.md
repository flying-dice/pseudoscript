# ADR-039 — Top-level `constant` is a primitive literal in a value namespace

**Status:** Accepted
**Affects:** LANG.md §2.3, §3, §8.1, §10

## Context

ADR-038 lets a body state a business rule (`if (x > LIMIT)`), but the threshold had
nowhere to live: there was no syntax for a named primitive value. A rule that
compares against a fixed limit had to inline a literal, losing the name the
business gives it.

## Decision

Add `constant Ident = Literal`:

- top-level only — a `constant` is a structural declaration, not a body statement
  or a node member;
- the value is a single primitive literal (`number`/`string`/`bool`); its type is
  inferred from the literal;
- `public` makes it addressable across modules (§8.2), like any structural
  declaration;
- referenced by FQN `module::PI` (ADR-030), which resolves to the declared
  primitive type;
- immutable and single-assignment (consistent with ADR-002);
- not evaluated — the value is type information for the checker and text for the
  diagrams, never a computed result.

Constant names occupy a new **value namespace** (§8.1), a fourth namespace beside
type, node, and feature names. A `data`, a node, a feature, and a constant MAY
share a name; two constants MUST NOT.

A constant is a value-position reference only as a full FQN `module::NAME`. A bare
leaf MUST NOT resolve to a constant (ADR-030), the same rule that governs nodes and
types.

## Consequences

- §2.3 adds `constant` to the keyword list.
- §3 gains a **Constants** subsection: `constant NAME = <primitive literal>`,
  top-level only, type inferred from the literal, `public` for cross-module, FQN
  reference `module::NAME`.
- §8.1 adds value names as the fourth module namespace; a constant FQN reference is
  value-position.
- §10 adds `Constant = "constant" Ident "=" Literal` and lists it under
  `Structural`.
- A `constant` whose right-hand side is not a primitive literal MUST be rejected; a
  macro on a constant MUST be rejected (the existing macro-target rule, §2.4).
- A bare leaf naming a constant does not resolve; only `module::NAME` does (ADR-030,
  ADR-036).
- Rejected alternative: a constant initialised by an operator expression
  (`constant LIMIT = 2 * 50`). Operators are never evaluated (ADR-038), so the
  declared value would not reduce to a literal; a constant carries one literal value
  and no computation.
