# ADR-037 — Architectural-principle lints are graph warnings with codes and article links

**Status:** Accepted
**Affects:** LANG.md §9.6

## Context

§8.2 visibility decides whether a reference *resolves*: a `public` node is
addressable across modules. It says nothing about C4 *structure* — whether the
call is a good idea. A public `component` is addressable from any module, yet
reaching it from outside its container backdoors the container's published face.
The repo already guarded its own models against that one case in a Rust test
(`facade_pattern_holds_in_every_workspace`), but the rule never surfaced to a user
authoring a `.pds` model.

The resolved graph (§9.1) already carries the facts these rules need: each `Call`
edge's source and target nodes, their `kind`, `module`, and the `for` ancestry
that gives each node its enclosing container and system. A lint over the graph
reaches every author through the existing diagnostic path.

## Decision

Three architectural lints run over the resolved graph, each emitting a `Warning`
(the model stays valid) stamped with a stable `PDS-ARCH-NNN` code and the URL of
its article (`docs/principles/`), which the LSP carries as `code_description` so
an editor renders the code as a link.

- **PDS-ARCH-001 — facade bypass:** a cross-module `Call` whose target is a
  `component`.
- **PDS-ARCH-002 — cyclic dependency:** a cycle in the module dependency graph
  (cross-module `Call` arcs `source.module → target.module`), reported once per
  strongly-connected component at a representative edge.
- **PDS-ARCH-003 — system-boundary bypass:** a `Call` crossing a `system`
  boundary whose target is a `container`.

## Consequences

- LANG.md §9.6 states the rules as `SHOULD`s and pins the `Warning` + code +
  article-URL contract.
- `syntax::Diagnostic` gains `code_description` (the article URL); `Edge` gains the
  call-site `span` so a warning points at the offending call. Both LSP edges
  (stdio `lsp-core`, the `pseudoscript-ide` wasm) carry the URL through to the
  editor.
- The lints run in the workspace and per-module check passes, so they appear live
  in the IDE problems pane and in `pds check`.
- Warnings are advisory: they never fail a check or block generation. A deliberate
  coupling (an anti-corruption layer at a system seam, a shared-kernel cycle the
  author accepts) ships with its warning visible rather than suppressed.
- The flagship samples carry real PDS-ARCH-002 cycles; they ship with the warnings
  rather than being rearchitected, demonstrating the lint on realistic models.
- Rejected alternative: make violations errors. A `component` being addressable but
  un-reachable would split visibility from callability and reject models that
  resolve cleanly; the C4 facade discipline is guidance, not a well-formedness rule.
- Rejected scope: a fourth "any cross-system container-to-container call" rule.
  Container-to-container relationships across systems are legitimate at the C4
  container level, so the rule produces false positives; PDS-ARCH-003 fires only on
  reaching *into* another system's container, and PDS-ARCH-001 covers the
  component case.
