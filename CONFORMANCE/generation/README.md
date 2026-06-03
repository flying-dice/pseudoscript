# Generation conformance

Diagram generation for `LANG.md` §9. A case is a model plus one golden per view it yields. Emission is SVG (the `pseudoscript-emit` backend, ADR-017), but raw SVG is brittle to golden — float coordinates, attribute order. So the assertion surface is the **`Scene` IR**: the laid-out, notation-neutral geometry the renderer turns into pixels. The golden pins *which* nodes / edges / participants / messages a model yields, in canonical order and containment — not pixel positions.

## File conventions

| Inputs | Expected output |
| --- | --- |
| `name.pds` | one `name.<view>.scene` per view the case exercises — e.g. `name.context.scene`, `name.container.scene`, `name.sequence.scene`. |

A case scoped to a node (container/component view) or a sequence entry encodes the target in a header line (`of` / `entry`), so one `.pds` can carry several view goldens. A single-file case is an anonymous module (ADR-029), as in `static/`.

## `Scene` IR golden format

One element per line, UTF-8, `\n`-terminated. The `Scene` is laid out by the emitter; the golden serialises it **without coordinates**. Lines appear in canonical order (below). Labels are quoted; the lexeme escapes `\` and `"`.

### C4 views (`context`, `container`, `component`)

```
view <context|container|component>
of <FQN>                                  # container/component only; absent for context
node <FQN> <kind> "<label>" [in <FQN>]    # kind: person|system|container|component; `in` names the boundary
edge <from-FQN> -> <to-FQN> <kind> "<label>"   # kind: call|trigger|provenance
```

- **Nodes** in source-declaration order. `<label>` is the node's simple name; a `///` summary, if any, does not appear in the `Scene` (it is renderer tooltip text, not structure).
- `in` is present when the node sits inside the view's boundary (a system's containers in a `container` view; a container's components in a `component` view), resolved via `for`.
- **Edges** after all nodes, sorted by `(from, to, kind, label)`. A trigger macro contributes one `trigger` edge from a synthesised initiator node — `event:<FQN>` (`#[onevent]`), `scheduler` (`#[schedule]`), `client` (`#[http]`), `caller` (`#[manual]`); a cross-boundary body call contributes a `call` edge; a `from` composition contributes a `provenance` edge.

### Sequence view (`sequence`)

```
view sequence
entry <FQN>                               # the triggered callable the trace starts from
participant <FQN>                         # lifelines, in order of first appearance
message <from-FQN> -> <to-FQN> <kind> "<label>"   # kind: call|return|self
frame <alt|loop> "<cond>"                 # opens a frame; nested lines indent two spaces
```

- **Participants** in order of first appearance in the trace; the entry's owner is first. A real trigger actor (client/scheduler/event) leads as its own lifeline and receives the entry's returns. A callable with no trigger omits the generic `caller` participant — the owner leads, and the entry's returns to that absent caller are suppressed (they reappear once a real incoming trigger exists).
- **Messages** in body evaluation order (§7). A chained expression emits one `message` per call, left-to-right; a `self.` call is `self` kind (from owner to owner). A `return` is a `return` message back to the caller. Every call has a matching return: a call to a disclosed callable expands inline and returns through its body; a call to a bodyless callable emits a synthesised `return ""` from callee to caller (the out-and-back response), its label empty and detail the callee's return type.
- **Frames**: `if`/`else` → `frame alt`; `for`/`while` → `frame loop`. The frame's body messages indent two spaces under it; a closing line is implicit at the dedent. An `else` arm emits a second `frame alt "else <cond>"`. An `if` whose then-arm ends in `return` and has no `else` is a guard clause: the steps following it in the same block run only when the guard is false, so they emit as that second `frame alt "else <cond>"`. A branch (then or else) whose traced body is empty — e.g. its only step is a return suppressed for a triggerless entry — emits no frame.

## Adding cases

- Goldens are **hand-written from the spec**, never dumped from the emitter — the spec leads (parent `README.md`). Match the canonical order above exactly.
- Keep a case minimal: one model exercising the view under test.
- Pixel-level SVG snapshots belong in `pseudoscript-emit`'s own tests, not here.

## What this layer does not pin

- Pixel coordinates, sizes, colours, attribute order — layout and styling vary by implementation.
- SVG document structure. The contract is the `Scene`; the SVG is its rendering.
