# Edge Cases & Resilience

The `Breaker` model captures the happy path of tripping and recovering. A reviewer's job is to ask what happens at the seams — the moments the three-line state machine glosses over.

## Edge cases & failure modes

**The half-open thundering herd.** `TripsOpenThenRecovers` says "a *single* trial call probes the feed." That word is load-bearing. If the breaker lets *every* waiting request through the moment the cool-down elapses, a recovering `FxFeed` gets slammed by the entire backlog at once and trips straight back open. The half-open state must admit exactly one probe (or a tight quota) and hold everyone else in `FeedDown` until that probe resolves.

**What counts as a failure?** `record(e: Timeout)` treats failures uniformly, but not all errors should trip the breaker. A `Timeout` or 503 means the feed is unhealthy — count it. A 400 (malformed `Pair`) or a 401 means *your* request is wrong; tripping on those punishes good traffic for a bug in one call. The model lumps everything into `record`; production code should classify.

**Clock skew on the cool-down.** "After a cool-down" assumes a reliable clock. If the breaker runs on multiple instances with drifting clocks, one node may consider the breaker open while another sends probes — defeating the single-probe guarantee. Worse, a clock jump can either end the cool-down early (premature probing) or never (stuck open).

**Slow success.** A `quote` that returns `Ok` after 9 seconds is technically a success but is exactly the latency you wanted to avoid. A breaker keyed only on errors won't trip on creeping slowness; you usually want latency itself to count as a failure.

## Resilience

The small model omits the *state* a real breaker needs. `isOpen` and `record` are signatures — behind them lives a failure counter, a trip timestamp, and the open/half-open/closed status, all of which must be shared correctly. In a distributed deployment that state belongs somewhere coherent (a shared store or a per-instance breaker that accepts some duplicated probing), not a single in-memory counter per node.

Harden it by: making the trip threshold a *rate* over a rolling window, not a raw count, so a slow trickle of failures doesn't trip a healthy feed; counting latency, not just errors; emitting a metric/event on every state change so the open breaker is visible on a dashboard; and tuning the cool-down with backoff so a feed that keeps failing its probe is retried less and less often.

## Pairs well with

- **Retry with backoff** — the breaker decides *whether* to call; retries handle the transient blips that don't warrant tripping. The breaker is the backstop when retries keep losing.
- **Bulkhead** — isolate the breaker-guarded feed in its own resource pool so even its slow calls can't starve the rest of the widget.
- **Fallback / cache** — `RateWatch`'s last-known rate is the fallback that makes failing fast actually graceful. A breaker without a fallback just fails faster.
- **Timeout** — a breaker is blind without aggressive timeouts; the timeout is what *defines* a failure for it to count.
