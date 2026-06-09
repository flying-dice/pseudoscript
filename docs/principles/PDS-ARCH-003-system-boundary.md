# PDS-ARCH-003 — System-boundary bypass

## BLUF

**A call that crosses a `system` boundary must target the other system's published
face, not reach into one of its `container`s.** Container-to-container calls
*within* one system are normal internal composition. But a call from one system
into a specific container of another system couples you to that system's internal
structure — you now depend on which container does what, so the other system can't
reorganise its containers without breaking you. Treat a foreign system as a black
box: call the `system` (its published interface), and let it route the request to
the right container behind its boundary. A gateway or anti-corruption layer at the
seam keeps the foreign system's shape from leaking into yours.

(Reaching a foreign *component* is the stronger violation — see
[PDS-ARCH-001](PDS-ARCH-001-backdooring-facade.md).)

## Example

The `Editor` system reaching directly into the `Lsp` container of the toolchain
system:

```pseudoscript
//! context
public system Editor {
  openDocument(doc: model::WorkspaceModule): void {
    // ✗ PDS-ARCH-003: Lsp is a container inside the Pseudoscript system.
    lsp::Lsp.onChange(doc)
  }
}

public system Pseudoscript {
  public onChange(doc: model::WorkspaceModule): void;
}
```

Call the system boundary; let it dispatch inward:

```pseudoscript
//! context
public system Editor {
  openDocument(doc: model::WorkspaceModule): void {
    // ✓ Pseudoscript is the published system face; Lsp stays its internal container.
    context::Pseudoscript.onChange(doc)
  }
}
```

## When it is acceptable

The seam component itself — a gateway / anti-corruption layer whose whole job is
to talk to the foreign system — will trip this rule. That is the one place the
coupling is deliberate; keep the warning visible there, or concentrate all foreign
calls in that single adapter so the rest of your system stays clean.

## Links

- `LANG.md §4` — the system → container → component nesting this rule reads.
- `LANG.md §9` — the architectural-graph rules this lint enforces.
- [decisions/037-architectural-lints.md](../../decisions/037-architectural-lints.md) — why these are warnings, and the rule rationale.
- [Anti-corruption layer (DDD)](https://learn.microsoft.com/azure/architecture/patterns/anti-corruption-layer), [Bounded context](https://martinfowler.com/bliki/BoundedContext.html).
