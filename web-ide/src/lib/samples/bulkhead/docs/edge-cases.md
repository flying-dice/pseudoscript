# Edge Cases & Resilience

`Wanderlust` cleanly isolates three pools, but the interesting questions in review are about what happens *at* the wall — and what the model leaves to the implementation.

## Edge cases & failure modes

**Pool sizing is the whole game.** The model says each pool is "bounded" but never says how big. Too small and `HotelsPool` returns `PoolFull` on a perfectly healthy provider during a normal traffic bump — you've manufactured the failure you were trying to contain. Too large and an exhausted pool holds so many hung connections that it exhausts a *shared* resource underneath (file descriptors, the database's connection cap), and the isolation leaks. The walls are only watertight if every pool plus its overhead fits the host.

**Queue vs reject.** When `HotelsPool` is full, does `query` reject immediately (as the model's `PoolFull` implies) or queue the request? Rejecting fast keeps latency bounded but drops load the moment you're busy. Queuing absorbs bursts but, unbounded, just relocates the stall — a 30-second queue wait is indistinguishable from a hang, and now the *queue* is the shared resource that backs up. A bounded queue with a short timeout is usually the sweet spot.

**The shared resource hiding below.** Three separate pools that all talk to the same upstream load balancer, DNS resolver, or TLS handshake pool aren't truly isolated. The hotel stall can still propagate through whatever they share. True bulkheading has to go all the way down.

**Caller fan-out semantics.** `Meta` dispatches three lanes, but the model doesn't say whether the `Traveller`'s search waits for all three or returns partial results. If it awaits all three, a `PoolFull` on hotels could fail the *whole* search unless `Meta` is written to degrade gracefully — return flights and cars, mark hotels unavailable.

## Resilience

The small model omits everything stateful: the actual pool size, the acquire timeout, the queue policy, and what `Meta` does with a partial result set. Harden it by sizing each pool from real concurrency × latency (Little's Law), giving every `query` a tight timeout so a stall *becomes* a `PoolFull` quickly instead of hanging, and having `Meta` return the lanes that succeeded rather than failing the whole search on one full pool. Emit per-lane saturation metrics — a pool that's chronically near full is your early warning.

## Pairs well with

- **Circuit breaker** — once `HotelsPool` is reliably full, trip a breaker on the hotel lane so calls fail instantly instead of waiting to be rejected; the bulkhead contains the damage, the breaker stops doing the damaging work.
- **Timeout** — the bulkhead only works if hung calls release their slots promptly; the timeout is what turns a hang into a freed connection.
- **Load shedding / fallback** — when a lane is full, serve a cached or partial result rather than an error.
- **Rate limiter** — cap inflow per lane so a single abusive caller can't be the one exhausting a pool.
