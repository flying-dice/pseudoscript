# ADR-034 — A `data` symbol projects an entity view; a `feature` projects a flow view

**Status:** Accepted
**Affects:** LANG.md §9.4, §9.5; references §3.5, §5.2

## Context

Selecting a symbol projects its fitting diagram (§9.1, §9.2): a system its containers, a callable its sequence. A `data` symbol had no fitting view — it fell back to the whole-model context overview, which says nothing about the type. A `feature` is not a graph node (§5.2), so selecting one projected nothing; the host's lifeline fallback then failed to lay out (`feature` is not a node kind) and the error escaped, crashing the canvas. Both the type's shape (§3.5) and the scenario's steps (§5.2) were modelled but unprojectable.

## Decision

A `data` symbol projects an **entity view** (§9.4); a `feature` projects a **flow view** (§9.5).

- **Entity view.** A card for the focal type plus the data types its fields reference, one hop out. A record renders one row per field, a union one row per variant, a black box no rows (§3.5). A row whose type resolves to another `data` type draws a reference arrow to that type's peer card. Resolution strips `[]`/generics, then matches an exact FQN, a module-qualified name, or any `data` of that simple name; a built-in type resolves to nothing.
- **Flow view.** A scenario's steps as connected nodes, top to bottom, in source order, naming the target node. Each node shows its keyword and prose.
- **Feature lookup.** A `feature` FQN is `module::name`; projecting it resolves the scenario among the graph's scenarios, since it is not a graph node (§5.2).

## Consequences

- §9.4 and §9.5 define the two views; the symbol-projection rule routes a `data` symbol to the entity view, no longer the context overview.
- The host no longer falls back to a single lifeline for a `feature`; the crash that fallback caused is removed.
- The graph carries each `data` node's disclosed shape (record fields / union variants / black box), lifted from the AST — the entity view reads it.
- Rejected alternative: reuse the C4 card for a data symbol. A C4 card shows kind/name/summary, not fields — it cannot render a type's shape.
- Rejected alternative: a generic single-lifeline fallback for any non-projectable symbol. It cannot lay out a `feature` (not a node kind) and produced the crash; a dedicated view is the fix.
