# Generation conformance — deferred

This layer is empty on purpose. It is PseudoScript's analog of an end-to-end
"run it and assert the output" layer: given a `name.pds` model, assert the
diagrams the generator must produce (`LANG.md` §9 — C4 Context / Container /
Component views, sequence diagrams, and data-flow/provenance views).

Emission is **pluggable** (`pseudoscript-emit`): a custom SVG renderer that owns
its layout (the headline — `LANG.md` §1, "has an SVG compiler"), plus
Dot/Mermaid/PlantUML text exporters — all drawing from the same resolved graph.
That makes goldening notation-dependent: the text exporters emit **deterministic
text**, so byte-for-byte goldens are fine for them; but raw **SVG** is brittle —
floating-point coordinates, attribute order, and layout tuning make it
implementation-bound, exactly what the parent `README.md` says expected outputs
must *not* be.

So the canonical, backend-independent assertion surface is the **`Scene` IR** —
the notation-neutral, laid-out geometry the SVG renderer turns into pixels
(positioned nodes, routed edges, frames, lifelines). It pins structure
deterministically (which nodes/edges/lifelines, in what order and containment)
without pinning pixels. Pixel-level SVG snapshots belong in implementation tests;
text-backend output, being deterministic, can be goldened directly once those
backends exist.

This layer waits on one thing: the `Scene` IR shape isn't specified yet — it's a
`pseudoscript-emit` design artifact, not a `LANG.md` construct. Once it
stabilises, add cases as `name.pds` plus one golden per view the model yields:

```
generation/9-container-view.pds
generation/9-container-view.container.scene       ← Scene IR for the C4 Container view
generation/9-sequence-openaccount.pds
generation/9-sequence-openaccount.sequence.scene  ← Scene IR for the entry trace
```

and a runner alongside the syntax/static runners.

Until then the generation contract is exercised indirectly: every `static/` case
proves a model is well-formed enough to lay out, and every `syntax/` case proves
it parses — a regression that breaks the model graph surfaces as a parse or
checker failure there.

> Note: many interesting generation cases (a Container view resolving children
> via `for`, cross-module diagrams) also need the multi-file workspace fixtures
> called out in the parent `README.md` "Known gaps". Both land together.
