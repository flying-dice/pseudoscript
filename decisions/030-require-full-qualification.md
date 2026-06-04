# ADR-030 — A node, type, or variant reference is always its FQN

**Status:** Accepted
**Affects:** LANG.md §3.4, §7, §8.1

## Context

A bare leaf name used to resolve to a node or union variant in the current module — `container Web for Store` found `Store` next to it, `feature X for Store` likewise. So a bare name had four things it could mean: a parameter, a binding, a `for` binding, or a same-module node/variant. The resolver had to try the local scopes, then fall back to the module's node and type namespaces, and a leaf that collided with a local read one way in a body and another in a `for`. The same name meant different things at different depths, and a reader could not tell a local from a node without knowing every declaration in the file.

Cross-module references were already full FQNs (`banking::core::Store`, §8.1); only the same-module case was short. Two spellings for the same target — `Store` here, `banking::core::Store` from next door — is one rule more than the FQN already expresses.

## Decision

Every reference to a node, type, or union variant MUST be its FQN, including a reference to one in the same module. A bare leaf name resolves only to a parameter, a binding, or a `for` binding.

- A bare name in a body resolves to a parameter, a binding, or a `for` binding — never a node or variant (§7).
- A `for` parent, a `feature` target, an `#[onevent]` event, a field/parameter/return type, and a generic argument each MUST be an FQN when they name a node, type, or variant (§3.4, §8.1).
- `self`, member access (§7.1), the primitives (§3.1), and `Result`/`Option` (§3.2) stay bare — they are not module-scoped names.

## Consequences

- LANG.md §8.1 states the rule; §3.4 and §7 lose "or a node" / "or a union variant" from bare-name resolution.
- The checker reports `` `Name` must be fully qualified: `module::Name` `` when a bare leaf names a same-module node, type, or variant. The rule is gated to a named module: a path-less anonymous single-file check (§8.1, ADR-029) has no module prefix to require and stays lenient; the workspace check, keyed on the real path FQN, enforces it.
- The worked model and the bundled samples are fully qualified throughout.
- Rejected alternative: keep the same-module short form. It is the convenient case, but it is the one that makes a bare name ambiguous between a local and a node, and it forces every tool to carry the namespace-fallback resolution the FQN sidesteps. One spelling per target is the simpler invariant.
