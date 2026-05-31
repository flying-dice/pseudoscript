# Edge cases & resilience

The peer-review pass: the failure modes that only show up under concurrency and partial
failure, and where in the model each is handled.

## One vendor sells out mid-checkout

*A basket spans three vendors; the second's stock is gone by checkout.* `Orders.place` takes
all holds in `holdAll` *before* charging. The failed `Catalog.hold` short-circuits with
`OutOfStock`, `releaseAll` returns the holds already taken on the other vendors, and the buyer
is never charged. The `SoldOutMidCheckout` feature pins this.

## Search shows an item that's actually gone

*The search index lags the write side, so a shopper baskets a just-sold item.* That's fine:
`Search` never gates a purchase. The authoritative `Catalog.hold` at checkout consults
`Inventory` directly, so a stale index can only cause a polite `OutOfStock` at checkout, never
an oversell.

## Payment captured but the order won't save

*The charge succeeds, then persistence or announcement fails.* The saga captures payment only
*after* every hold is taken, and a capture failure path (`releaseAll` + `Declined`) returns the
holds. Money never leaves without the order being recorded; a failure before capture leaves no
charge at all.

## Two buyers reach for the last unit

*Concurrent checkouts contend on one listing's last unit.* The reservation rests on
`Inventory.reserve`, an atomic check-and-reserve on a small counter split out from the listing.
Exactly one wins; the other gets `OutOfStock`. No-oversell is enforced at the data layer.

## A shipment event delivered twice

*The fulfilment system re-emits `SubOrderShipped` after a retry.* `Payouts.onShipped` is an
idempotent receiver: it checks `Dedup.seen` and a sub-order already processed is a no-op. The
vendor is credited exactly once. The `IdempotentReceipt` feature pins this.

## A crashed payout drain

*The outbox relay dies mid-transfer.* Because `Ledger.credit` commits the ledger entry and the
outbox row together, and `Outbox.drain` is idempotent on payout id and reads only
`OutboxStore.undelivered`, a crashed-then-retried drain never pays a vendor twice and never
loses a payout.

## A vendor suspended with open orders

Suspending a vendor hides their new listings but is modelled as a `VendorStore` concern, not a
checkout one — in-flight orders already placed continue through the saga to fulfilment and
payout. A suspension stops new sales without stranding paid buyers. (Suspension mechanics
beyond activation are a deliberate scope cut.)

## The deliberate scope cuts

Connection pools, retry/timeout policy on the providers, the exact search ranking, and tax /
shipping calculation are tuning and plumbing concerns, omitted on purpose. Every money step and
every stock decision survives in the model.
