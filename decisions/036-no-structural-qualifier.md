# ADR-036 — A structural-qualifier path is not an FQN; only the flat FQN resolves

**Status:** Accepted
**Affects:** LANG.md §8.1

## Context

A node reference is its flat FQN `module::Name` (§8.1, ADR-030). The toolchain
carried a leaf-fallback that quietly allowed a second spelling: a node addressed
through its C4 ancestry — `Syntax::Parser` (container `Syntax`, component
`Parser`), or cross-module `convp::ConversationPlatform::Instacom::InstacomExchange`
— resolved by matching its last segment against the module's nodes.

The fallback was not uniform, so the spellings diverged across surfaces:

- goto/hover/inference resolved a drill by the unique workspace leaf;
- the graph builder resolved it only when the leaf was a same-module node;
- the cross-module checker flagged neither — a drill was neither a bare name nor a
  recognised dangling FQN.

A cross-module drill therefore passed the checker, built a graph edge to a node
that does not exist, and broke the C4 container/component diagram — while goto and
the sequence view appeared to work. One input, four inconsistent outputs.

## Decision

Only the exact flat FQN `module::Name` resolves. A structural drill —
`Container::Component` or `module::System::Container::Component` — names no node.

- The leaf-fallback is removed from the graph builder (`canonicalize`) and the
  cursor resolver (`resolve_node`); a multi-segment path resolves only as an exact,
  visible symbol.
- The checker reports the drill and suggests the flat FQN, preferring the
  same-module node the drill's local qualifier names.

## Consequences

- LANG.md §8.1 states a structural drill is not an FQN and MUST NOT resolve.
- The checker emits `` <role> `<drill>` is not a fully-qualified name; use `<flat>` ``;
  a multi-segment path whose leaf names no node is reported as a dangling or
  unresolved reference.
- goto/hover/inference no longer paper over a drill — the diagnostic surfaces it.
  Bare-name leniency for an under-qualified single name (navigation only) is
  unchanged; that is ADR-030's concern.
- Single-file/anonymous mode (ADR-029) has no module to qualify against and stays
  lenient.
- The worked model and the bundled samples are migrated to flat FQNs throughout.
- Rejected alternative: keep the structural-qualifier convenience because it reads
  like a C4 path. It is a second spelling for every target and the source of the
  silent phantom-edge divergence above; one spelling per target is the simpler
  invariant (reinforces ADR-030).
