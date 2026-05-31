# Trip lifecycle & CQRS

The `Trips` container (in `trips`) is the spine of the model. Everything upstream exists to
create a trip; everything downstream reacts to its events.

## A guarded lifecycle

A trip moves through explicit states, and each transition is a guarded, disclosed callable:

- **`Trips.accept`** turns an `Offer` into a `Trip`. It first reads `OfferStore.active` —
  an expired offer returns `None` and the acceptance is rejected with `OfferExpired`. Only a
  live offer creates a trip. The new trip is saved and a `TripStarted` event is appended to
  the log.
- **`Trips.complete`** finalises an in-progress trip. It guards on `TripStore.inProgress`;
  completing a trip that is not in progress is rejected with `NotInState`. On success it
  appends `TripCompleted` — the trigger for settlement in `payouts`.

Illegal transitions are rejected at the guard, returning a variant of `TripError`. The trip
can never, say, be completed before it has started.

## Event sourcing over the log

Trip state is not a field to be overwritten — it is **derived**. Every transition appends an
event to `TripLog` (a `#critical`, append-only container whose `append` is the only writer).
The log, not a mutable snapshot, is the source of truth. That buys auditability (the full
history of every trip is reconstructable for disputes and fraud review) and cheap, rebuildable
read models.

## CQRS read model

`TripHistory` is the read side. Its `onCompleted` callable is triggered by
`#[onevent(TripCompleted)]` — it subscribes to the log rather than calling the write
aggregates, and records each completed trip into `HistoryStore`. Nothing on the read path
touches `Trips`, `TripStore`, or `OfferStore`.

This is the textbook CQRS payoff: the history view can be scaled, cached, and rebuilt from the
log independently of the write path, and a momentary lag in the read model can never corrupt
the authoritative trip state.

## Handing off to money

`complete` appends `TripCompleted`, and that event is the boundary where the trip lifecycle
ends and the money lifecycle begins — `payouts` consumes the very same event.
