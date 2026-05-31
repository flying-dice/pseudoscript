# The Checkout Saga

A purchase spans three systems that can each fail independently: inventory (the seats), the payment provider (the money), and ticketing (the credentials). There's no distributed transaction across them — you can't two-phase-commit a third-party card charge. So `orders.pds` models the buy as a **saga**: a sequence of local steps, each with a compensating action, driven by a durable state machine that survives a crash at any point.

## The state machine

An `Order` carries an `OrderStatus`, and the intermediate states are *durable progress markers*, not transient flags:

```
Pending  → Charged → Confirmed
   │                     
   └──────────────→ Failed / Refunded   (terminal)
```

`Pending` is written *before any money moves*. `Charged` means the card was captured. `Confirmed` means seats are allocated and tickets are issuing. The point of persisting each step is recovery: a crash leaves the order at its last saved state, and the `batch::SagaSweeper` resumes it from exactly there. Forward progress, never a guess.

## Reserve, then check out

Reservation comes first. `Reservation.hold` runs the gauntlet: verify waiting-room admission, authenticate the session, confirm the event is on sale, enforce the per-order quantity cap (anti-scalping), then take an `inventory::Holds.reserve` — an atomic, no-oversell allocation. Finally it quotes the price and **locks it to the hold** (`pricing::Locks.lock`). If pricing fails after the seats are taken, it releases the hold before returning — the first compensation in the flow.

Then `OrderService.checkout` runs the saga proper:

1. **Re-verify admission and authenticate.** The admission token is checked again — the gate is not just at reservation.
2. **Idempotency check.** `OrderStore.byHold` — a hold has at most one order. If one already exists, return it; a retry never creates a second order or a second charge. This is backed at the data layer: `idFor(hold)` is a *deterministic* id, so two concurrent checkouts of the same hold collide on one order id, never two.
3. **Validate and commit the hold.** `Holds.commit` moves the hold `Held → Committing`, which protects it from the reaper while payment is in flight. A charge whose outcome is unknown can't have its seats reaped out from under it.
4. **Persist `Pending`, then charge.** The order is saved *before* `payments::PaymentService.charge` is called, so a crash mid-charge leaves a durable record to reconcile against.
5. **Confirm the hold into tickets.** On a successful charge, mark `Charged`, then `Holds.confirm` converts the committed hold into an `Allocation`, and `OrderConfirmed` is raised — the event `tickets`, `notifications`, and `pricing::Demand` each consume independently.

## The compensations

Every money step has a reversal, and choosing the *right* reversal for each failure is the whole craft:

- **Card declined → release the hold.** A definite decline means no money moved. `afterChargeFailure` releases the seats back to the pool and marks the order `Failed`. (`DeclineReleasesHold`.)
- **Allocation fails after charging → refund.** If the card was captured but `Holds.confirm` then fails, the charge is reversed with `PaymentService.refund`, the hold released, and the order marked `Refunded`. The buyer is made whole. (`AllocationFailureRefunds`.)
- **A refund that itself fails → surface, don't swallow.** If the reversal fails, the checkout returns `RefundFailed` and the order is **not** marked refunded — the money is owed and the failure is visible, not silently dropped.

## The one branch that doesn't compensate

The sharpest decision: an **indeterminate** charge failure — a provider timeout where the money may or may not have moved — does *not* release the hold or fail the order. `afterChargeFailure` returns `PaymentUncertain` and leaves the order `Pending` with the hold still committed. Treating a timeout as a decline is exactly how a charged buyer loses their seat with no record. Instead, the `SagaSweeper.resume` reconciles the real outcome against the provider later: a confirmed charge drives the order forward (`complete`), a charge the provider *definitively* reports as never taken abandons it (`abandon`), and a still-unknown outcome is left for the next sweep. The order is never abandoned while its charge might be real. Every branch is idempotent, so resuming a saga any number of times is safe.

This is the core promise of the design, stated as a state machine: a buyer is never left charged-without-tickets, and seats are never left held-without-a-buyer.
