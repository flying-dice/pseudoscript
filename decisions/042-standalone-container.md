# ADR-042 — Standalone container: `for` is optional, parentless renders at the context layer

**Status:** Accepted
**Affects:** LANG.md §4, §9.1, §10
**Amends:** ADR-010

## Context

ADR-010 made `for <parent>` mandatory on every `container`: a container's parent MUST be a `system`. Modelling a flat set of containers at a single architectural plane — no meaningful system breakdown — then forced a stub `public system X;` as an anchor purely to satisfy the hierarchy. The stub carries no information yet pollutes the outline and the rendered context diagram with an empty node.

## Decision

- **`for` is optional on a `container`.** `public container Foo {}` parses and passes `check`. A `component` still MUST name its parent.
- **A parentless container is standalone** — a top-level node at the context layer (§9.1), alongside `person` and `system`. It is not omitted and does not error.
- **The kind rule survives.** A container's `for` parent, *when named*, MUST be a `system`; a component's MUST be a `container`. Any other parent kind MUST be rejected.

## Consequences

- §10: `Container = "container" Ident [ "for" Path ] Body ;`.
- §4: a container MAY omit `for`; a parentless container is standalone.
- §9.1: the context view (and the `pds doc` index) lists standalone containers beside persons and systems. Context-view edges bubble to the nearest in-view ancestor (the enclosing system, or the standalone container itself for its descendants), not to a system specifically.
- The parser drops the missing-`for` error for containers; it is retained for components.
- Rejected alternative: the stub-system workaround (`public system X;` as an anchor) — it misrepresents the architecture and clutters the diagram.
