# Wayfare

Wayfare is an on-demand ride-hailing platform. A rider opens the app, asks for a ride,
and within seconds a nearby driver is on the way at a price both sides accept. Behind
that simplicity is a real-time matching market with money on both ends.

## The hard part

Ride-hailing is a matching market under time pressure, with money on the line:

- **Supply and demand move every second.** Drivers go online and offline; requests spike
  at rush hour. Matching must be fast and local.
- **Price must clear the market.** When demand outruns supply, surge pricing rations
  scarce capacity and pulls more drivers online — but the price must not move under a
  rider mid-checkout.
- **Money must be exact.** A completed trip pays a driver and takes a platform fee. A
  half-finished payout is a support ticket at best.
- **Downstreams are flaky.** The maps provider and the payment provider are third parties
  that time out. A blip must never strand a rider or double-pay a driver.

## The architecture at a glance

The C4 context (`context.pds`) names the platform `Wayfare`, two actors — `Rider` and
`Driver` — and three external systems it does not own: a `MapsProvider`, a
`PaymentProvider`, and a `PushProvider`. The work is split into bounded contexts, each
its own module:

- **`shared`** — the value objects, payloads, and event/error families every context uses.
- **`dispatch`** — `Dispatch` matches a request to a nearby driver (`DriverIndex`) and
  prices it (`Pricing` + `SurgeIndex`), producing a locked `Offer`.
- **`trips`** — the `Trips` lifecycle, the append-only `TripLog`, and the `TripHistory`
  CQRS read model.
- **`payouts`** — the `Payouts` saga that settles a completed trip and the `Outbox` that
  delivers transfers reliably.

## How to read this model

Start with **dispatch** (how a trip gets created and priced), then **trips** (the lifecycle
and the read model), then **payouts** (how a completed trip turns into money). The two
deep-dive docs and the edge-cases review walk the headline flows.

## Patterns on display

- **Surge pricing** — `Pricing.quote` scales a base fare by `SurgeIndex`, locked to the offer.
- **Event sourcing** — `TripLog` is the append-only truth; trip state is a fold over it.
- **CQRS** — `TripHistory` projects `TripCompleted` into a read model, off the write path.
- **Saga** — `Payouts.onTripCompleted` settles money with per-step compensation.
- **Transactional outbox** — `Outbox` commits with the ledger and drains transfers at-least-once.
