# Checkout & payouts

One basket, many vendors, one payment — and later, many payouts. This is the part of a
marketplace that has to be exactly right with money.

## Checkout as a multi-vendor saga

`Orders.place` (triggered by `#[http("POST /orders")]`) runs the checkout as a compensating
saga:

```
hold every line's stock          compensate: release every hold
capture one payment for the total compensate: refund payment
save the order + announce it
```

The ordering is deliberate. **All holds are taken before any money moves.** `holdAll` reserves
stock for every line through `Catalog.hold`; if even one line is out of stock it short-circuits
with `OutOfStock`. Only once every line is reserved does `PaymentProvider.charge` capture a
single payment for the basket total. If that capture fails, `releaseAll` returns every hold and
the order is rejected with `Declined` — the buyer is never charged for a half-filled basket.
On success, an `OrderPlaced` event is published for fulfilment.

The `PlaceOrder` and `SoldOutMidCheckout` features pin the happy path and the rollback.

## Payouts through an outbox

The buyer paid once, but the marketplace now owes each vendor their share. When a sub-order
ships (`SubOrderShipped`), `Payouts.onShipped` runs — triggered by `#[onevent(SubOrderShipped)]`.

Two hazards are handled here:

- **Duplicate delivery.** Upstream events are at-least-once, so `onShipped` is an **idempotent
  receiver**: it checks `Dedup.seen` first and a sub-order already processed is a no-op. The
  `IdempotentReceipt` feature pins this.
- **The dual-write hazard.** Crediting a vendor means writing a ledger entry *and* triggering a
  bank transfer. `Ledger.credit` writes the ledger entry and a paired `Outbox` row **in one
  transaction**, so a crash can't record a payout without queuing it. `Outbox.drain` (scheduled)
  then delivers undelivered rows through `PaymentProvider.transfer`, idempotent on payout id.

The result: every vendor payout is recorded exactly when owed and delivered exactly once, even
across crashes and retries. The ledger and the bank can never disagree.
