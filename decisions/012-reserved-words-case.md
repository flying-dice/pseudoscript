# ADR-012 — Reserved words and case sensitivity

**Status:** Accepted
**Affects:** LANG.md §2.2, §2.3, §10

## Context

The spec never stated which names are reserved, nor whether identifiers are case-sensitive or bound by an enforced casing convention.

## Decision

- **Reserved words.** All §2.3 keywords, the primitive type names (`number`, `string`, `bool`, `datetime`, `uuid`, `void`), `Result`, `Ok`, and `Err` are reserved and MUST NOT be used as identifiers. `data string` / `data Ok` MUST be rejected. (ADR-019 extends the set with `Some`, `None`, and the type name `Option`.)
- **Case sensitivity.** Identifiers are case-sensitive: `Banking` and `banking` are distinct.
- **Casing convention.** PascalCase for nodes/data/types and lowercase for locals/params is convention only; the checker MUST NOT enforce it.

## Consequences

- §2.2: states case sensitivity and the convention (non-normative).
- §2.3 / §10: the reserved set spans keywords, primitives, `Result`, `Ok`, `Err`.
- Rejected alternatives: keywords-only reservation; enforced casing.
