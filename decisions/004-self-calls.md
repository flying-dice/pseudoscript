# ADR-004 — Self/sibling calls via `self.`

**Status:** Accepted
**Affects:** LANG.md §2.3, §5, §10

## Context

Every call in the spec is cross-node-qualified (`AccountStore::Repository.fetch`). Invoking a callable that lives on the *same* node (a sibling, or itself for recursion) had no notation.

## Decision

- `self` refers to the enclosing node (the `system`/`container`/`component` that owns the callable).
- A same-node callable is invoked as `self.Name(args)`.
- `self` is a keyword, valid only inside a callable body.
- Bare unqualified `Name(args)` does **not** resolve to a same-node callable; the `self.` qualifier is required.

## Consequences

- §2.3: `self` added to the keyword list.
- §5: documents `self.Name(args)` for sibling and recursive calls.
- §10: `Ref` admits `self`.
- Sequence diagram: a `self.` call renders as a self-message on the node's lifeline.
