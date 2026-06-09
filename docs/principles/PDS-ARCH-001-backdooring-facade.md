# PDS-ARCH-001 — Facade bypass (backdooring)

## BLUF

**Don't reach into another container's internal `component` from outside its
module — call the container's published face instead.** A `component` is a
private internal of its container. When a node in one module calls a `component`
declared in another module, it backdoors the container: it couples to an
implementation detail the container never published, so the container can't
change its internals without breaking the caller. Route the call through the
container (or system) — the facade/gateway — and let it delegate inward.

This is the C4 equivalent of the **Facade** and **Gateway** patterns: a boundary
object that presents a stable, intention-revealing surface and hides the parts
behind it.

## Example

A `gateway` module reaching straight into the `charges` container's internal
`ChargeStore` component:

```pseudoscript
//! gateway

public container Gateway for acme::Payments {
  capture(id: string): Result<charges::Receipt, string> {
    // ✗ PDS-ARCH-001: ChargeStore is an internal component of the charges container.
    return charges::ChargeStore.persist(id)
  }
}
```

Go through the `charges` container's published callable — the frontdoor:

```pseudoscript
//! gateway

public container Gateway for acme::Payments {
  capture(id: string): Result<charges::Receipt, string> {
    // ✓ charges::Charges is the container's published face; it owns ChargeStore.
    return charges::Charges.capture(id)
  }
}
```

```pseudoscript
//! charges

public container Charges for acme::Payments {
  // The published face delegates inward to its own components.
  public capture(id: string): Result<charges::Receipt, string> {
    return self.store.persist(id)
  }
}

// ChargeStore stays an internal component — never named from another module.
component ChargeStore for charges::Charges {
  persist(id: string): Result<charges::Receipt, string>;
}
```

## Links

- `LANG.md §8.2` — visibility: a `public` component is *addressable* across
  modules, but addressability is not an invitation to reach in.
- `LANG.md §9` — the architectural-graph rules this lint enforces.
- [decisions/037-architectural-lints.md](../../decisions/037-architectural-lints.md) — why these are warnings, and the rule rationale.
- [Facade pattern](https://en.wikipedia.org/wiki/Facade_pattern), [Gateway (Fowler, PoEAA)](https://martinfowler.com/eaaCatalog/gateway.html).
