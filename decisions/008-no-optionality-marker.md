# ADR-008 — No optionality marker (`?` removed)

**Status:** Accepted
**Affects:** LANG.md §3.3, §3.4, §10
**Amends:** ADR-001 (which had kept `?` as an optionality marker)

## Context

ADR-001 removed `Option` but retained `?` as a standalone optionality marker. With no Option type and no instantiation, `?` only annotated whether a field could be absent — a concept thin enough to question. A field is a field; cardinality, if ever needed, can live in prose or tags.

## Decision

- `?` is removed from the language. There is no optionality marker.
- `[]` (array) is the only type suffix.
- `Type = Named [ "[]" ]`.

## Consequences

- §3.3: drop `?`; a type is `Named` with an optional `[]`.
- §3.4: example field `owner: Person?` becomes `owner: Person`.
- §10: `Type = Named [ "[]" ]`.
- The `?`/`[]` ordering question (would-be ADR-008b) is void.
- Supersedes the `?` clause of ADR-001.
