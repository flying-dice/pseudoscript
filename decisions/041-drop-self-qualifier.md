# ADR-041 — Same-node calls are bare; `self.` is dropped

**Status:** Accepted
**Supersedes:** ADR-004
**Affects:** LANG.md §3.3, §5.1, §8.1, §9.2, §10

## Context

ADR-004 gave a same-node callable the notation `self.Name(args)` and reserved bare `Name(args)` as non-resolving. The qualifier became the only notation for a sibling or recursive call.

It also became a silent modelling trap. A `self.` call renders as a collapsed self-message: the renderer draws the self-arrow but does not follow the callee's body, so any cross-boundary call inside that body never reaches the sequence diagram. A callable `postMessage` delegating to `sendEvent`, whose body is `Topic.produce(event)`, rendered the self-arrow to `sendEvent` and dropped the Kafka publish entirely — with no diagnostic. The model passed `pds check`; the downstream interaction was invisible (issue #71).

A direct call (`Node.method(args)`) does not have this problem: the renderer follows a disclosed callee's body and emits its calls (§9.2). The divergence was the `self.`-specific collapse.

## Decision

- A same-node callable is invoked by a bare call `Name(args)` — a sibling, or the enclosing callable itself for recursion. The `self.` qualifier is removed.
- A bare name **in call position** (immediately followed by `(`) resolves to a callable on the enclosing node. A bare name not in call position is unchanged: it resolves only to a parameter, a binding, or a `for` binding (§8.1, ADR-030).
- `self` stays a reserved word (§2.3) so the parser diagnoses the removed `self.` form and points at the bare call. `self` MUST NOT appear in a model.
- Sequence rendering: a same-node call renders as a self-message **and** the renderer follows the callable's body inline, emitting its cross-boundary calls, exactly as a direct call to a disclosed callee does. Recursion is stack-guarded — an in-flight callee renders as a single self-message with no expansion (§9.2). A method on a local value or chain intermediate (`x.f()`) stays a leaf self-message; it names no node callable and has no body to follow.

## Consequences

- §10: `Ref` drops `self`; a bare call `Name "(" [ Args ] ")"` is added as a primary.
- §5.1: the same-node-call clause is rewritten — bare call, recursion included.
- §8.1: a bare name in call position resolves to a same-node callable; the value-position rule is unchanged.
- §9.2: a same-node call expands its callee's body; the silent drop is gone.
- The checker resolves a bare call to the enclosing node's callables and applies the same arity and argument-type checks as any call (ADR-022, ADR-023); a name matching no callable on the node is reported as unresolved.
- The implementation splits the trace step: a same-node call follows its body, a local-value method stays a leaf — both still render as self-messages.
- The worked model, the bundled samples, the conformance cases, and the authoring skill are migrated to the bare form.
- Rejected alternative: keep `self.` and only fix the renderer to follow its body (issue #71's fallback). The qualifier carries no information the enclosing scope does not already supply, and its collapsed self-message was the source of the silent drop. A bare call is indistinguishable from any other call in the model, so the renderer follows it under one rule — one call shape, one rendering. Two spellings for one call is the divergence ADR-036 settled against.
- The call-vs-construction ambiguity does not arise: construction is `Type from { … }` (§7.2), never `Name(args)`; a bare name followed by `(` is unambiguously a call.
