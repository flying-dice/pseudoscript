// The starter model shown on first load — exercises persons, systems,
// containers, components, a `data`, a `feature`, and a triggered callable with
// a body (so every diagram view has something to show).
export const SAMPLE = `//! shop

/// A person browsing and buying.
/// #external
public person Customer;

/// A line item in a basket.
public data Item { sku: string, qty: number }

/// The storefront customers interact with.
public system Storefront;

/// Serves the web UI and the checkout flow.
public container Web for Storefront {
  /// Render the catalogue.
  get(): void;

  /// Place an order for the basket.
  #[http]
  public Checkout(item: Item): Result<number, string> {
    ok = Payments.Charge(item)
    if (ok.isOk) {
      return Ok(ok.value)
    }
    return Err("payment failed")
  }
}

/// Takes and settles payments.
public container Payments for Storefront {
  /// Charge a single item.
  public Charge(item: Item): Result<number, string>;
}

/// Checkout settles a paid basket.
feature SettleCheckout for Web {
  given "a basket with one item"
  when "the customer checks out"
  then "the payment is charged"
  and "an order number is returned"
}
`;
