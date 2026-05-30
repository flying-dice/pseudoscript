# ADR-018 — `feature` BDD scenarios: prose given/when/then for a node

**Status:** Accepted
**Affects:** LANG.md §2.3, §5.2, §8.1, §9.3, §10, §11

## Context

The spec described structure (§4) and behavior (§5) but had no way to record a node's expected behavioral scenarios. A BDD-style `feature ... for <node>` documents one scenario as a given/when/then flow, surfaced on the node's doc page (§9.3).

## Decision

- **Top-level, node-targeted.** `feature` is a top-level construct. `feature Name for Path` names the node the scenario is about; `Path` resolves as an FQN (§8) to a `system`, `container`, `component`, or `person`. A target resolving to a type or module MUST be rejected; a cross-module target MUST be `public` (§8.2, per ADR-010).
- **Prose steps.** Each step is a step keyword followed by a string literal. The string is descriptive prose, not resolved against the model — features add no C4 or sequence edges.
- **One scenario per feature.** A `feature` block is a single given/when/then flow, not a container of named scenarios.
- **Strict Gherkin flow.** Zero or more `given`, then one or more `when`, then one or more `then`, in that order. A `then` before any `when`, or a `when` after any `then`, MUST be rejected. `and`/`but` continue the preceding step's kind; a leading `and`/`but` MUST be rejected. The flow is encoded in the grammar (§10), so violations are parse-level rejects.
- **Namespace.** Feature names occupy a third module namespace (§8.1), distinct from type and node names; a name MUST be unique among the module's features.
- Extends ADR-012: `feature`, `given`, `when`, `then`, `and`, `but` join the reserved set.

```pds
feature OpenAccount for banking::core::Mainframe {
  given "a verified owner"
  when  "the owner opens an account"
  then  "banking info is returned"
}
```

## Consequences

- §2.3: six keywords reserved.
- §5.2: the construct, its flow rules, and the node-target rule.
- §8.1: feature names are a third namespace.
- §9.3: each feature renders as a scenario card on its target node's section.
- §10: `Program` admits `Feature`; the `Feature`/`FeatureBody`/`Given`/`When`/`Then`/`Cont` productions encode the strict flow.
- Rejected alternatives:
  - **Structured-statement steps** (steps wrapping calls/assignments/Result accessors, wired into the sequence diagram). Rejected: a scenario's value is the prose contract; structured steps duplicate the disclosed body (§5.1) and couple documentation to resolution.
  - **Hybrid steps** (a prose label plus an optional structured body). Rejected: same coupling, with two ways to write one step.
  - **Multi-scenario features** (`feature { scenario { ... } }`, Gherkin-faithful). Rejected: one feature == one flow keeps the construct flat, matching the language's flat-structure principle (§1).
  - **Loose flow** (require the three kinds but not their order, or `when`+`then` only). Rejected: the user requirement is to enforce the Gherkin flow.
