# PDS-ARCH-002 — Cyclic dependency

## BLUF

**Modules must not form a dependency cycle.** When module A calls into B, B into
C, and C back into A, the three can no longer be understood, tested, or deployed
independently — they are one tangled unit wearing three names. Cycles also hide
the real layering: nothing is "above" or "below" anything. Break the cycle by
extracting the shared concept into its own module both sides depend on, or by
inverting one direction (the callee depends on an abstraction the caller
implements).

This is the **Acyclic Dependencies Principle**: the dependency graph of releasable
units must be a DAG.

## Example

`orders` calls `billing`, and `billing` calls back into `orders` — a 2-cycle:

```pseudoscript
//! orders
public container Orders for shop::Shop {
  place(cart: orders::Cart): Result<orders::Order, string> {
    // ✗ orders depends on billing …
    return billing::Billing.charge(cart)
  }
  public priceOf(order: orders::Order): orders::Money;
}
```

```pseudoscript
//! billing
public container Billing for shop::Shop {
  public charge(cart: orders::Cart): Result<orders::Order, string> {
    // ✗ … and billing depends back on orders. Cycle.
    return orders::Orders.priceOf(cart)
  }
}
```

Extract the shared concept (`pricing`) both depend on — the cycle becomes a DAG:

```pseudoscript
//! pricing
public container Pricing for shop::Shop {
  public priceOf(cart: orders::Cart): orders::Money;
}
```

```pseudoscript
//! orders
public container Orders for shop::Shop {
  place(cart: orders::Cart): Result<orders::Order, string> {
    return billing::Billing.charge(cart) // orders → billing
  }
}
```

```pseudoscript
//! billing
public container Billing for shop::Shop {
  public charge(cart: orders::Cart): Result<orders::Order, string> {
    return pricing::Pricing.priceOf(cart) // billing → pricing, never back to orders
  }
}
```

Now `orders → billing → pricing`, acyclic.

## Links

- `LANG.md §9` — the architectural-graph rules this lint enforces.
- [decisions/037-architectural-lints.md](../../decisions/037-architectural-lints.md) — why these are warnings, and the rule rationale.
- [Acyclic Dependencies Principle](https://en.wikipedia.org/wiki/Acyclic_dependencies_principle), [Dependency Inversion Principle](https://en.wikipedia.org/wiki/Dependency_inversion_principle).
