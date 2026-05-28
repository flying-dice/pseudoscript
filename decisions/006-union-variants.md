# ADR-006 — Union variants: inline declares and hoists, bare references

**Status:** Accepted
**Affects:** LANG.md §3.5, §8.1, §10

## Context

`Variant = Ident [Record]` leaves the record optional, raising two questions: is a bare `| Name` a new fieldless variant or a reference to an existing `data`, and where does a variant's name live once declared?

## Decision

- `| Name { ... }` **declares** the variant and **hoists** it to the enclosing module namespace, addressed as `module::Name` — the same as a top-level `data`.
- Bare `| Name` (no record) **references** an existing module-level `data Name`.
- A reference whose target does not exist MUST be rejected.
- A declared variant whose name collides with another module-level `data` MUST be rejected.

```pds
data BankAccCreated { accId: string }      // standalone
data AccountEvent =
  | BankAccCreated                         // reference
  | BankAccClosed { accId: string }        // declare + hoist → banking::events::BankAccClosed
```

## Consequences

- §3.5: documents declare-vs-reference and module-level hoisting.
- §8.1: variant names share the module namespace with `data` declarations.
- §10: `Variant = Ident [ Record ]` retained; semantics pinned here.
- Rejected alternatives: variants scoped under the union (`Union::Name`); inline-only with no references.
