# ADR-010 — `for` parent: FQN addressing and kind rules

**Status:** Accepted
**Affects:** LANG.md §4, §8.2, §10

## Context

`Container = "container" Ident "for" Ident Body` (and the `Component` equivalent) used a bare `Ident` for the parent and constrained its kind nowhere. This blocked cross-module parenting and allowed nonsensical parent kinds.

## Decision

- **Addressing.** The parent is a `Path`, resolved as an FQN (§8). A container or component MAY be parented to a `public` node in another module; a private cross-module parent MUST be rejected (per §8.2).
- **Kind.** A `container`'s parent MUST be a `system`. A `component`'s parent MUST be a `container`. Any other parent kind MUST be rejected.

```pds
container Ledger for banking::core::Banking { }   // cross-module system parent
```

## Consequences

- §10: `Container = ... "for" Path Body ;` and `Component = ... "for" Path Body ;`.
- §4: states the two kind rules.
- §8.2: cross-module parent must be `public`.
- Rejected alternatives: same-file-only parents; nested components; no kind constraint.
