# Surge Volume & Surge Pricing

Two different things spike when a hot show goes on sale: the *number of people* arriving, and the *demand* those people represent. ACME Tickets answers each with its own mechanism — the virtual waiting room for volume, surge pricing for demand — and they meet at one point: the moment you reserve.

## Shaping the volume: the waiting room

If ten thousand requests hit the reservation endpoint at once, the contention isn't on the network — it's on the last seat, the payment provider's rate limit, the database row everyone wants. The fix is not to scale the checkout path to ten thousand concurrent buyers. It's to never let ten thousand arrive at once.

That's `waitingroom`. An `Attendee` doesn't reserve directly; they `Queue.join`, which returns a `QueueTicket` and a place in line. Admission is the gate every reservation and checkout must pass — `Admission.verify` runs before anything expensive happens.

The engine is `Gatekeeper.admitNext`, driven once a minute by the `batch::Drainer`. Each tick, it computes *headroom* and admits exactly that many queued attendees, minting each an `AdmissionToken` and alerting them. The whole design lives in how headroom is computed:

```
headroom = max(0, seats_available - admissions_outstanding)
```

`seats_available` comes from `inventory::Capacity.snapshot` — inventory owns the seat truth. `admissions_outstanding` is the count of tokens already granted but not yet spent, owned by the waiting room. Admission is the *conservative minimum* of the two. Because each is read a moment apart, the loop can only ever **under**-admit, never over — it can grant fewer tokens than seats, but never more. A buyer is admitted only when there is genuinely room for them.

There's a subtlety the model is careful about: an admitted buyer is "outstanding" until their hold shows up in the capacity snapshot, then stops. `AdmissionStore.outstandingAsOf` keys the count to the snapshot version, so a buyer is never subtracted twice — once as a token and again as a not-yet-visible hold — which would otherwise make the gate throttle forever and converge to admitting nobody.

One deliberate non-guarantee: the queue controls admission *rate*, not seat *order*. Among admitted buyers, seats are assigned best-available across shards (`QueueGatesRateNotSeatOrder`). True seat-FIFO would re-serialise the very allocation that sharding exists to spread.

## Pricing the demand: surge, locked at hold time

Surge pricing is `pricing`. A `Quote` is the tier's base price (`catalog::Tiers.priceOf`) scaled by a `SurgeFactor` the `Quotes` component derives from a live `Demand` snapshot — sell-through, remaining inventory, queue depth folded into a pressure index, then mapped through a tuning curve. High pressure lifts the multiplier above one; as demand cools it eases back toward one. The *composition* is disclosed (price = base × surge(demand)); the demand model itself is a black box the pricing team tunes.

Two design choices make surge safe rather than hostile.

**It fails open.** If the demand signal is unavailable, `surgeFor` falls back to a neutral, no-surge factor and the sale proceeds. A non-critical pricing signal never blocks a purchase.

**It's locked at hold time.** This is the fairness guarantee. When you reserve, `orders::Reservation.hold` calls `pricing::Locks.lock(holdId, quote)` — the price you were quoted is frozen to your hold. At checkout, `OrderService` reads it back with `Locks.priceFor`; if the lock is gone, checkout refuses rather than re-pricing. So even if surge doubles in the three minutes you spend entering your card, you pay what you were quoted. The price cannot jump mid-checkout, by construction.

Demand ingestion is kept off the hot path entirely: `Demand.onConfirmed` reacts to the same `OrderConfirmed` event tickets and notifications consume, folding sales into the aggregate asynchronously — never a synchronous write on the path it prices. And because that event is delivered at-least-once, `Demand` de-duplicates by order id (`DemandLog`): folding a sale twice would lift the surge every later buyer pays, so the consumer owns its own idempotency rather than trusting the producer.

Together: the waiting room decides *who gets to the checkout and how fast*; surge pricing decides *what they pay* and freezes it. Volume and demand, handled separately, meeting once at the hold.
