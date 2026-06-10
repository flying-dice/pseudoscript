# ADR-040 — Mandatory return types

**Status:** Accepted
**Affects:** LANG.md §5.1, §10

## Context

The return type was optional; absence meant `void` (`F()` ≡ `F(): void`). Two spellings of the same signature, and an omitted type gave the return checks (ADR-016 coverage, ADR-020/022 type match) nothing to bite on. Parameters were already strict (`Param = Ident ":" Type`) — the asymmetry was only on the return side. No prior ADR pinned the optionality.

## Decision

- Every callable MUST declare a return type — disclosed and black-box forms alike. A callable without one MUST be rejected.
- `void` is the explicit nothing-spelling: `F(): void`. No new syntax; `void` stays a primitive (§3.1) and stays composable in generics (`Result<void, E>`).
- §10: `Callable = Ident "(" [ Params ] ")" ":" Type ( Block | ";" )` — the `[ ":" Type ]` brackets drop.
- `Ok`/`Err`/`Some`/`None` remain value-only; one in type position (`a(): None`) MUST be rejected — they are keywords (§2.3), not type names, so the type fails to parse.
- The parser stays infallible: a missing return type yields a syntax diagnostic and the tree recovers with `void`.
- Checking scope is unchanged (ADR-022's conservative inference stands), but mandatory signatures make a call to a resolvable callable a determinable `return` operand: the §5.1 type-match clause now reaches a call whose callee resolves within the module — `self.Method()` (ADR-035's reach, previously `from`-sources only) or a same-module node receiver. Cross-module callee resolution stays deferred (ADR-022's boundary).

## Consequences

- §5.1: the mandatory-return-type clause replaces the optionality clause.
- §10: the `Callable` production requires `":" Type`.
- Conformance: `syntax/5-callable-no-return-type.reject`, `syntax/5-callable-blackbox-no-return-type.reject`, `syntax/3-marker-as-return-type.reject`, `static/5-return-call-type-mismatch`.
- Every `.pds` surface (samples, `model/`, skill, starter) writes `: void` where it previously omitted the type.
- Precedent: Java/C# mandate `void` in every signature — the reader base this language targets. Rust defaults to `()` and allows omission, but pairs omission with full inference; this language's signatures are the architecture surface, so omission costs information.
- Rejected alternative: keep implicit `void` for black-box stubs only — a black box is exactly where the return type is the only information disclosed.
- Rejected alternative: full static typing of every `return` operand — reverses the shape-hints stance (§1); ADR-020/022 already rejected it.
