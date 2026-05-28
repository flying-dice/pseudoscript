# ADR-015 — Per-macro target constraints

**Status:** Accepted
**Affects:** LANG.md §2.4, §10

## Context

The grammar let a macro attach to any declaration, while §2.4 described every macro as a trigger on a callable. Rather than a single blanket rule, each macro governs its own valid attachment: some macros target callables, others could target containers or other kinds.

## Decision

- Each macro in the closed built-in set declares the declaration kind(s) it may attach to.
- A macro on a declaration kind outside its target set MUST be rejected.
- The grammar permits a macro syntactically on any declaration (`Decl` and `BodyMember`); the checker enforces per-macro targeting.
- The current built-in set is the four trigger macros — `#[onevent]`, `#[schedule]`, `#[http]`, `#[manual]` — and every one targets **callables**.

## Consequences

- §2.4: the macro table gains a target column; the "every macro is a trigger" line is scoped to the current built-ins, leaving targeting a general property.
- §10: `{ Macro }` stays on both `Decl` and `BodyMember`; targeting is a static rule, not a grammar rule.
- Rejected alternatives: macros on callables only; macros freely on any declaration with no target check.
