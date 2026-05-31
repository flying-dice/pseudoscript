# Edge cases & resilience

The peer-review pass: the failure modes that only show up under load and partial failure, and
where in the model each is handled.

## No driver, or a maps outage

*The rider requests a ride and nothing is nearby — or the maps provider is down.*
`Dispatch.requestRide` separates these: `DriverIndex.nearest` returning `None` yields
`NoDriverAvailable`, while `MapsProvider.eta` returning `Err` yields `MapsDown`. Neither
leaves a request dangling, and the rider is told which happened. The maps provider's
flakiness is a first-class branch, not a crash.

## Surge whiplash

*Raw demand/supply ratios are spiky.* `SurgeIndex.multiplierFor` is documented to clamp and
smooth, so a momentary spike doesn't quote a 9x fare. And because `Pricing.quote` locks the
multiplier into the `Fare` carried by the `Offer`, the price the rider confirms is the price
they pay — it cannot move under them mid-checkout.

## Accepting an expired offer

*A driver taps "accept" a beat too late.* `Trips.accept` reads `OfferStore.active` first; an
expired or already-accepted offer returns `None`, so the acceptance is rejected with
`OfferExpired` and no trip is created. Two trips can never spring from one offer.

## Completing a trip that isn't running

*A retry or a confused client calls complete twice.* `Trips.complete` guards on
`TripStore.inProgress`; a trip not in progress returns `NotInState`. The second completion is
a rejected no-op, so only one `TripCompleted` event is appended — and therefore only one
payout is ever accrued.

## Trip completes but settlement fails

*The card is captured, then the driver-ledger credit fails.* This is the dangerous gap.
`Payouts.onTripCompleted` is a saga: it captures the platform fee, and if `Ledger.credit`
returns `Err`, it **refunds the captured fee** before returning the error. Each step that can
fail names its compensation, so a partial failure never leaves the driver shorted or the
platform double-paying.

## A crashed payout drain

*The outbox relay dies mid-transfer — did the money move?* `Outbox.drain` is triggered on a
schedule and reads only `OutboxStore.undelivered`, marking each row delivered after a
successful `transfer` (and failed otherwise). Because the ledger entry and the outbox row are
committed together, and `drain` is idempotent on payout id, a crashed-then-retried drain
never pays a driver twice and never loses a payout.

## The deliberate scope cuts

Connection pools, retry/timeout policy on the providers, push-notification fan-out, and the
exact surge curve are deployment and tuning concerns, not architecture — they are omitted on
purpose. What survives in the model is every decision and every money step.
