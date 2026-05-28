# ADR-013 — Literal forms and placement

**Status:** Accepted
**Affects:** LANG.md §2.2, §2.3, §7, §10
**Extends:** ADR-012 (reserved words)

## Context

§10 referenced `Literal` in `Expr` and `Meta` without defining it, and the static model raised the question of whether literals belong in bodies or only in macro arguments.

## Decision

- **Forms.** `Literal` is a string (`"..."`), a number (digits with optional decimal), or a bool (`true` / `false`).
- `true` and `false` are reserved words.
- **Placement.** Literals MAY appear in macro arguments and as call arguments (e.g. `Repo.page(0, 20)`). `Literal` stays in `Expr`.

## Consequences

- §10: define `Literal = String | Number | Bool ;` with `Bool = "true" | "false"`; `Literal` retained in `Expr` and `Meta`.
- §2.2 / §2.3: `true` and `false` added to the reserved set (extends ADR-012).
- §7: call arguments may be literals.
- Rejected alternatives: string-only literals; literals confined to macro args.
