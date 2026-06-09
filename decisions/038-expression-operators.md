# ADR-038 — Arithmetic, comparison & boolean operators are type-checked, never evaluated

**Status:** Accepted
**Affects:** LANG.md §1, §7, §10, §12

## Context

§12 #3 parked the expression grammar: conditions admitted only `Ref`/call/`!Expr`,
with no comparison or boolean operators, and arithmetic had no syntax at all. A
business rule like "reject when the balance exceeds a limit" could not be stated as
code — `if (x > module::LIMIT)` did not parse, and `return x / 2` did not parse.

ADR-019/§1 framed bodies as *flow and provenance, not field-level computation*. A
threshold comparison or a halving is computation over primitives, which that stance
forbade in syntax while the diagrams already needed the condition text.

## Decision

Add the full operator set, parsed and **type-checked but never evaluated** (no
interpreter):

- arithmetic `+ - * / %`,
- comparison `< > <= >=`,
- equality `== !=`,
- boolean `&& ||`,
- unary `-` (negation) alongside the existing unary `!`.

Operators compose by precedence (lowest to highest): `||`, `&&`, `== !=`,
`< > <= >=`, `+ -`, `* / %`, unary `! -`, then postfix. `Marker` (`Ok`/`Err`/…)
and a `from` expression are expression heads (§10) and do not combine with binary
operators; binary operators compose over the value tier (postfix / literal / ref).

Static type rules, applied where both operands are determinable (ADR-022):

| Operators | Operand rule | Result | On determinable mismatch |
|-----------|--------------|--------|--------------------------|
| `+ - * / %` | both `number` | `number` | reject |
| `< > <= >=` | both `number` | `bool` | reject |
| `== !=` | both the same primitive | `bool` | reject |
| `&& \|\|` | both `bool` | `bool` | reject |
| unary `-` | `number` | `number` | reject |
| unary `!` | `bool` | `bool` | reject |

Conservative inference (ADR-022) holds: any `Unknown` operand makes the result
`Unknown` and fires no check. A constant FQN reference (ADR-039) resolves to its
declared primitive type, so `if (x > module::LIMIT)` types as `bool`.

This resolves §12 #3 and supersedes the "still open" note in ADR-023: a condition
MAY now be a binary expression yielding `bool`, checked by the same §7 rule.

## Consequences

- §1 relaxes "bodies describe flow and provenance, not field-level computation" to
  admit static business-rule computation over primitives and constants. Bodies are
  still never executed.
- §7 gains an **Operators** subsection: the precedence table, the operand/result
  rules above, and the note that operators are static.
- §10 layers the expression tier into a precedence cascade
  (`OrExpr → AndExpr → EqExpr → RelExpr → AddExpr → MulExpr → UnaryExpr`); `Marker`
  and `FromExpr` stay heads that do not combine.
- §12 #3 is struck.
- The checker rejects a determinable non-`number` arithmetic/comparison operand, a
  non-`bool` boolean operand, and equality across mismatched primitives; it stays
  silent on any `Unknown` operand.
- The sequence-diagram `alt`/`loop` frame label renders a binary condition through
  the same expression printer.
- Rejected alternative: evaluate operators (fold constants, run conditions). The
  language models architecture, not behaviour; a value the checker computes is a
  value a reader would expect to be live. Operators stay descriptive — type-checked
  so a rule is well-formed, never run.
