# ADR-011 — Block contents: person behavior, callables-only blocks

**Status:** Accepted
**Affects:** LANG.md §1, §4, §5, §9, §10

## Context

The uniform `Body = "{" { BodyMember } "}" | ";"` raised two questions: whether a `person` may hold behavior (§4 had said it owns none), and what a disclosed `system`/`container`/`component` block may contain.

## Decision

- **Person owns behavior.** A `person` MAY own callables that model actions it initiates (e.g. `MakePurchase`). This overturns the earlier "a person owns no behavior" rule. A person is `;` (black box) or a block of callables, like any node.
- **Blocks hold callables only.** A disclosed `system`/`container`/`component`/`person` block contains callables (§5) and nothing else. Containers and components are top-level declarations wired by `for` (ADR-010); they never nest inside a block.

```pds
public person Customer {
  MakePurchase(item: Sku): Result<Receipt, PurchaseError>;
}
```

## Consequences

- §4: drop "owns no behavior"; a person may disclose callables.
- §1: "behavior lives with its owner" now includes persons.
- §9: a person's callables are valid sequence-diagram participants / entry points.
- §10: `Body` content stays `{ BodyMember }`; `BodyMember = ... Callable` for every structural kind including `person`.
- Rejected alternatives: person empty-only; person `;`-only; nested components inside blocks.
