# ADR-042 — Standalone nodes: `for` is optional on container and component; parentless renders at the context layer

**Status:** Accepted
**Affects:** LANG.md §4, §9.1, §9.6, §10
**Amends:** ADR-010

## Context

ADR-010 made `for <parent>` mandatory on every `container` and `component`: a container's parent MUST be a `system`, a component's MUST be a `container`. Modelling a flat set at a single architectural plane — no meaningful enclosing breakdown — then forced a stub `public system X;` (or stub container) as an anchor purely to satisfy the hierarchy. The stub carries no information yet pollutes the outline and the rendered context diagram with an empty node.

## Decision

- **`for` is optional on a `container` and a `component`.** `public container Foo {}` and `public component Bar {}` parse and pass `check` (the warning below aside).
- **A parentless node is standalone** — a top-level node at the context layer (§9.1), alongside `person` and `system`. It is not omitted and does not error.
- **The kind rule survives.** A `for` parent, *when named*, MUST be a `system` for a container and a `container` for a component. Any other parent kind MUST be rejected.
- **The container is the canonical flat-grain primitive.** A standalone `component` is structurally a standalone `container` — both render as one context-layer box. Offering both as silent equals would let a model encode one structure two ways, so a parentless component raises **PDS-ARCH-004** (§9.6): an advisory `Warning` steering it to a `container` or a `for <container>`. The grammar is uniform; the redundancy is judged on the graph, not at the parser.

## Consequences

- §10: `Container = "container" Ident [ "for" Path ] Body ;` and `Component = "component" Ident [ "for" Path ] Body ;`.
- §4: either MAY omit `for`; a parentless node is standalone; the container is canonical.
- §9.1: the context view (and the `pds doc` index) lists standalone containers and components beside persons and systems. Context-view edges bubble to the nearest in-view ancestor (the enclosing system, or the standalone node itself for its descendants), not to a system specifically.
- §9.6: PDS-ARCH-004 joins the architectural lints — the first that judges a **declaration**, not a `Call` edge.
- The parser drops the missing-`for` error for both kinds.
- Rejected alternatives: (a) the stub-system/stub-container workaround — it misrepresents the architecture and clutters the diagram; (b) a per-model "don't mix standalone containers and components" lint — a whole-graph constraint coupling unrelated declarations, firing across independent modules; the redundancy is local, so it is judged per-declaration instead; (c) keeping `for` mandatory on a component — it leaves the flat-component case to a stub container.
