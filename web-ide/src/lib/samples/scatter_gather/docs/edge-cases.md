# Edge Cases & Resilience

`Compare.quote` reads like three tidy calls and a `best`. In production the three calls are the whole story — because they don't all come back on time, and sometimes one doesn't come back at all.

## Edge cases & failure modes

**The slow or failing provider.** `InsurerC.price` is a call to a system QuoteMesh does not own. It can hang, error, or rate-limit. If `Compare` waits forever, one stuck insurer freezes every comparison. Each leg needs a timeout, and a slow leg must be treated as a non-answer, not a reason to block.

**Partial results vs all-or-nothing.** Once timeouts exist you face a design choice the model leaves open: if `InsurerC` times out, does `best(a, b, c)` fail the whole request, or return the best of `a` and `b`? For price comparison, two real quotes beat zero — degrade to a partial result. For a quorum read or a "must hear from everyone" vote, partial is wrong and the request must fail. Decide deliberately; the right answer is domain-specific.

**Tail latency is the slowest provider.** Scatter-gather's response time is not the average insurer's latency — it is the *slowest* one you still wait for. Fan out to more providers and you are more likely to hit a slow tail on every request. This is the defining cost of the pattern: your p99 is somebody else's p99, amplified by N.

**The aggregator as a bottleneck.** Every request funnels through `Compare`. It holds N open connections per request, fans them out, waits, reduces. Under load it is the first thing to saturate, and if it falls over, no quotes flow at all — a single point of failure dressed as a coordinator.

**Fan-out amplification.** One `Shopper.compare` becomes three `price` calls. A thousand shoppers become three thousand insurer hits. Each insurer sees QuoteMesh's full traffic multiplied, which can trip *their* rate limits and turn into cascading slowdowns right when you are busiest.

## Resilience

Bound every leg with a timeout and treat the timeout as a result. Where it fits, hedge — fire a second request to a slow insurer rather than wait. Add per-provider circuit breakers so a chronically failing `InsurerB` is skipped fast instead of dragging the tail. Cache recent `Quote`s for identical `Cover` to cut fan-out amplification. Make `best` total over whatever subset of `a, b, c` actually returned, so a partial gather still yields an offer.

## Pairs well with

Scatter-gather leans on the **circuit breaker** and **timeout/retry** patterns to tame slow providers, on a **bulkhead** to keep one insurer's failure from exhausting `Compare`'s connection pool, and on a **cache** to absorb repeated identical fan-outs. When the workers must be discovered rather than hard-wired, a **service registry** feeds `Compare` the current list of insurers.
