# Edge Cases & Resilience

This is the peer-review pass: the subtle failure modes that only show up under concurrency and partial failure, and where in the model each is handled or bounded. These are the questions an architecture review actually asks — "what happens if…" — answered against the real nodes.

## Overselling under concurrency

*Two buyers reach for the last seat in the same millisecond.* The guarantee can't rest on luck. It rests on `inventory::Pool.allocateFrom`, an **atomic** take from one seat shard: two requests for the last seat cannot both succeed (`NoOversell`). Sharding (`SeatShard`, `allocateSharded`) spreads contention across N independent atomic writers for throughput, but a request only fails when the *whole tier* is sold out — `allocateAny` falls back across shards — so the speed-up never weakens the guarantee. Total capacity is the sum of the shards; no-oversell holds per shard and therefore overall.

## Double-charge on retry

*A buyer hammers "pay"; a load balancer retries; the saga sweeper re-runs.* Three layers stop a second charge. First, `OrderStore.idFor(hold)` is a deterministic id, so concurrent checkouts of one hold collide on a single order id (and thus a single charge key) at the database. Second, `checkout` returns the existing order on `byHold` before doing anything. Third, `PaymentService.charge` returns the existing `receiptFor` without re-calling the provider, and every provider call carries an `IdempotencyKey` so the provider dedupes too (`ChargeIsIdempotent`). The card is charged once.

## A webhook arriving before the charge is recorded

*The provider's settlement webhook races ahead of our own ledger write.* `settle` only applies to a payment that exists and is **awaiting settlement**; if the payment record isn't there yet, the webhook is acknowledged and dropped. The lost settlement is not lost forever — the pull `Reconciler` later finds the unsettled order and converges it. Push and pull together mean neither ordering nor delivery has to be perfect.

## A hold reaped mid-checkout

*The reaper expires a hold at the same instant the buyer is paying for it.* This is prevented by the `HoldStatus` machine. `commit` moves a hold `Held → Committing` *before* the charge, and the reaper's `expire` only touches holds still in `Held` (`ensureHeld`). A hold being paid for has already left `Held`, so the reaper can't release seats out from under a charge in flight. The `ensureHeld` / `ensureCommitting` checks are the compare step of a compare-and-set that makes commit, confirm, and expire mutually race-safe. And `confirm` is idempotent — an already-`Confirmed` hold returns its allocation — so the saga's forward-recovery can retry it.

## A refund that itself fails

*Support refunds an order, but the provider refund call fails.* The model refuses to lie about money. `OrderService.refund` reverses the charge **first**, and only releases the seats and marks the order `Refunded` if that succeeds. A failed reversal returns `RefundFailed`; the order stays `Confirmed` and the seats stay sold (`RefundFailureSurfaced`). The money owed is visible, never silently written off.

## The charged-but-un-ticketed buyer

*The card is captured, then ticket issuance fails.* Two backstops. If issuance fails *during* the saga (`Holds.confirm` errors), the charge is refunded and the order rolled back (`AllocationFailureRefunds`). If issuance fails *after* confirmation — a poison `OrderConfirmed` event — `tickets::Issuer` quarantines the order (`Quarantine.stranded`) instead of retrying forever, and a `SupportAgent` drains it via `backoffice` (`reviewStranded`). A charged buyer with no tickets becomes a human work item, never a silently-lost message.

## The indeterminate timeout

*The charge call times out — did the money move or not?* Covered in depth under the saga and payments docs, but it's the keystone: `afterChargeFailure` leaves an uncertain charge `Pending` rather than failing it, and `reconcileCharge` resolves it against provider truth. The order is never abandoned while its charge might be real.

## At-least-once events, consumer-owned idempotency

`Orders::Events` publishes `OrderConfirmed` **at-least-once** through a transactional outbox — the strongest a producer can honestly offer. So every *consumer* owns its dedup: `pricing::DemandLog` (lest a double-counted sale inflate surge for everyone after), `payments::EventLog`, and ticket issuance keyed by allocation id. The producer never assumes consumer de-dup; the contract is disclosed on both sides.

## The deliberate scope cuts

A good model says what it *won't* do. The reviewed cuts: asynchronous refund settlement (a `Refunding` state + refund webhook) is omitted because refunds are off the hot path and rarely contended; seat-level FIFO is omitted because the queue guarantees admission *rate*, not seat order, and FIFO would re-serialise the sharded allocation; shard cardinality and drain cadence are deployment knobs, not model concerns. Each cut is named where it's made, with the reasoning — so a reader knows it was a decision, not a gap.
