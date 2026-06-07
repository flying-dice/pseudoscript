# ADR-028 — Drop `alias`

**Status:** Accepted
**Affects:** LANG.md §1, §2.2, §2.3, §2.5, §7, §8, §10

## Context

`alias Name = Path;` bound a file-local short name to a node FQN (former §8.3) — `alias Store = banking::core::AccountStore;`, then `Store.fetch(id)`. It was pure sugar: every alias rewrote to the FQN it stood for, adding no reference a full name could not already make. It carried real cost, though. A bare name now had a fourth thing to resolve to (param, binding, node, *alias*), so the resolver followed alias chains with a cycle bound, and an alias whose name collided with a node muddied which the cursor meant. It also leaned on the very name-vs-namespace ambiguity that brittle resolution comes from: `alias Core = banking::core;` had to be rejected as "a module, not a node."

## Decision

`alias` is removed from the language. A cross-reference is written as its fully-qualified name: `banking::core::AccountStore` in-workspace, `dep::module::Node` across a dependency (§8.3).

- `alias` is no longer a keyword (§2.3); it lexes as an ordinary identifier.
- The grammar drops the `Alias` production and the `{ Alias | … }` alternative (§10).
- A bare name in a body resolves to a parameter, a binding, a node, or a union variant — not an alias (§7).

## Consequences

- LANG.md §8 loses its `alias` subsection; the former §8.4/§8.5 (Dependencies, Resolution & lockfile) renumber to §8.3/§8.4.
- The lexer drops `KwAlias`; the AST drops `Item::Alias` and `Alias`; the resolver drops alias-following (and its cycle bound); the checker drops the dangling-alias / alias-target-is-a-module diagnostics; completion and semantic tokens drop the alias entry.
- Conformance: `static/8-alias-to-module`, `static/8-dangling-alias`, `syntax/8-alias-missing-semi`, `syntax/8-alias-to-callable`, and `syntax/8-modules-alias` are removed; the `lexical/2-2-paths-colon-vs-dot` case demonstrates `::`/`.` with a `for`-parent path instead.
- Rejected alternative: keep `alias` as optional sugar. A second naming mechanism for the same target is one more rule every reader and tool carries, for a shorthand the FQN already expresses. Fully-qualified everywhere is the simpler invariant.
