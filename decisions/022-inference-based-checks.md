# ADR-022 — Inference-based body checks: references, member access, return types, call arity

**Status:** Accepted
**Extends:** ADR-020 (return-type checking)
**Affects:** LANG.md §2.2, §3.4, §5.1, §7, §8

## Context

The checker validated structure and control flow but did almost nothing inside callable bodies: an undefined name, a `.field` that does not exist, a return of a mistyped binding, and a wrong-arity call all passed clean. These violate rules the spec already implies (§2.2 `.` reads a field of a resolved value; §5.1 calls match their parameters; §7/§8 names resolve), but the implementation never enforced them.

## Decision

A conservative type inference underpins four body checks. Inference yields a concrete type only for statically-determinable forms — literals, `Ok`/`Err`/`Some`/`None` markers, `from` (incl. `Type[] from`), and bare parameter/binding references; calls, field accesses, `self`, and `::` paths yield `Unknown`. A check never fires on an `Unknown`.

- **Reference resolution (§7/§8).** A bare single-segment name must resolve to a parameter, a binding, a `for` binding, a node, an alias, or a union variant (a fieldless variant is referenced by name). An unresolved reference MUST be rejected.
- **Return-type inference (§5.1, extends ADR-020).** A `return` whose operand infers to a concrete type — now including a parameter or binding — MUST match the declared return type.
- **Member access (§2.2/§3.4).** A `.field` read whose receiver infers to a same-module `data` record (with disclosed fields) MUST name one of its fields.
- **Call arity (§5.1).** A call whose receiver resolves to a same-module node (`self` or a node name) MUST pass exactly as many arguments as the callable declares.

## Consequences

- `Member` gains `param_types` (the callee's parameter types, for arity).
- Each check is TDD-backed (`references`/`type_checking`/`member_access`/`call_arity` cucumber features) with conformance fixtures.
- Scope boundary (not done): cross-module member/arity resolution, argument-*type* checking, chained-receiver inference (`a.b.c`), and `if`/`while` condition typing. These need deeper inference and are deferred.
- A fieldless union variant is a valid value reference (it produces the variant); the reference check accounts for this.
- Rejected alternative: full static typing of every expression — too large a reversal of the shape-hints stance (§1); the checks enforce only the determinable forms.
