# ADR-005 — Unhandled Result is allowed

**Status:** Accepted
**Affects:** LANG.md §6, §7, §9.2

## Context

A fallible call may appear as a statement whose `Result` is neither bound nor inspected:

```pds
AccountStore::Repository.delete(id)   // returns Result<void, E>, Err branch ignored
```

§6 ("errors handled explicitly with `if`") suggests rejecting this; §1 progressive disclosure ("fill in only the flows worth tracing") suggests allowing it. PseudoScript is a modeling notation, not a compiler enforcing exhaustiveness.

## Decision

- A call statement MAY ignore its `Result`.
- A dropped `Result` is not an error and produces no diagnostic.
- The call still renders as a sequence-diagram message.

## Consequences

- §7: a `Call` statement is valid regardless of return type.
- §6: explicit `if` handling is the idiom, not a requirement.
- Rejected alternatives: mandatory handling; warning on unconsumed `Result`.
