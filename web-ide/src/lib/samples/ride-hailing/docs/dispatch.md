# Dispatch & pricing

Dispatch is where supply meets demand. A rider's `RideRequest` arrives, and `Dispatch`
turns it into a priced `Offer` by finding a driver and locking a price — or it fails,
explicitly, with the reason.

## Matching

`Dispatch.requestRide` (in `dispatch`) is the disclosed entry point, triggered by
`#[http("POST /requests")]`. It reads the closest online driver from `DriverIndex`
(a `#critical` spatial index, black-boxed behind a `nearest` signature). The two ways it
can fail are kept distinct and explicit:

- **`NoDriverAvailable`** — `DriverIndex.nearest` returns `None`. The request is rejected
  cleanly; no offer and no trip are created.
- **`MapsDown`** — `MapsProvider.eta` returns `Err`. The third-party maps provider is flaky,
  so its failure is a first-class branch, not an exception that bubbles into the rider's face.

Both arms return a variant of the `DispatchError` union (`shared.pds`), so the caller — and
the eventual HTTP controller — knows exactly which happened.

## Pricing the scarcity

Only once a driver and an ETA are in hand does `Pricing.quote` run. It computes a base cost
from distance and time (`baseCost`, black-boxed pricing detail) and scales it by the live
surge multiplier from `SurgeIndex.multiplierFor`. The resulting `Fare` is composed with
`from { req, base, multiplier }` and **locked into the `Offer`** — the rider sees a stable
number, and it cannot climb while they confirm.

Surge does two jobs at once: it rations scarce drivers to the riders who value them most,
and it raises driver pay to pull more cars online. Keeping `SurgeIndex` a separate container
isolates the economics of scarcity from the mechanics of matching, so each can be tuned
independently.

## Why the split

Matching answers *who*; pricing answers *how much*. Splitting them keeps each rule small and
testable — and the `feature` scenarios `RequestRide` and `NoDriver` pin the two outcomes
that matter as an acceptance contract.
