# ADR-007 — Full `.` chaining

**Status:** Accepted
**Affects:** LANG.md §7, §9.2, §10

## Context

The grammar permitted a single `.` per expression (`Call = Ref ["." Ident] "(" ... ")"`, `FieldAccess = Ref "." Ident`), forcing an assign-then-read idiom. This blocked field-of-field, field-off-call, and call chains.

## Decision

- `.` access and call chaining are unrestricted: `a.b.c`, `Repo.fetch(id).value`, `a.f().g()` are all valid.
- A chain evaluates left-to-right.

## Consequences

- §10: replace the single-`.` `Call`/`FieldAccess` productions with a left-recursive postfix form, e.g.
  ```ebnf
  Postfix = Primary { "." Ident [ "(" [ Args ] ")" ] } ;
  ```
- §9.2 sequence mapping: each *call* in a chain is its own message, emitted in left-to-right order; field accesses between calls are local and emit no message.
- §7: a chained expression is still one statement.
- Rejected alternatives: single-`.` ceiling; field-off-result only.
