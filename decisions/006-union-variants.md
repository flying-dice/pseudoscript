# ADR-006 — Union variants: inline declares and hoists, bare references

**Status:** Accepted
**Affects:** LANG.md §3.5, §8.1, §10

## Context

`Variant = Ident [Record]` leaves the record optional, raising two questions: is a bare `| Name` a new fieldless variant or a reference to an existing `data`, and where does a variant's name live once declared?

## Decision

- `| Name { ... }` (record variant) **declares** the variant and **hoists** it to the module's type namespace, addressed as `module::Name` — the same as a top-level `data`.
- Bare `| Name` (no record) **references** an existing module-level `data Name`.
- A declared variant whose name collides with another module-level type name MUST be rejected.

```pds
data BankAccCreated { accId: string }      // standalone
data AccountEvent =
  | BankAccCreated                         // reference
  | BankAccClosed { accId: string }        // declare + hoist → banking::events::BankAccClosed
```

## Refinement — fieldless variants

A bare `| Name` whose target does not exist is **not** an error (the original "reference whose target does not exist MUST be rejected" is withdrawn). When no module-level `data Name` exists, bare `| Name` declares a **fieldless variant scoped to the union**: it does not hoist, so its name MAY repeat across unions and MAY coincide with a node name. This is what makes enum-style unions (`Severity = | Error | Warning | Info`) legal without a backing `data` per variant.

Only record variants enter the type namespace; the collision rule applies there.

## Consequences

- §3.5: documents reference-or-declare for bare variants, and record-variant hoisting.
- §8.1: type names (`data` + hoisted record variants) and node names are distinct namespaces.
- §10: `Variant = Ident [ Record ]` retained; semantics pinned here.
- Rejected alternatives: variants scoped under the union (`Union::Name`); inline-only with no references.
