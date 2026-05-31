# Edge Cases & Resilience

`SkyCast`'s `withinBudget` check is one line in the model. Behind that line is a small distributed-systems problem, and that's where review attention belongs.

## Edge cases & failure modes

**Distributed counters.** The model implies one budget per `apiKey`, but `SkyCast` almost certainly runs on many instances. If each instance keeps its own counter, a key's *real* budget is `N × budget` — and a flooder spraying across instances gets `N` times the allowance. A correct limiter needs shared state (Redis, a central counter) or a coordinated algorithm. That shared store is now on the hot path of every request, with its own latency and failure modes.

**Burst vs sustained.** "Per-minute budget" hides a choice. A fixed window resets on the minute boundary, so a caller can fire a full budget at 11:59:59 and another full budget at 12:00:00 — a 2× burst across the seam. A token bucket (which the comment names) smooths this by refilling continuously and is the right call; a sliding-window counter is the other common fix. The model says token bucket but a reviewer should confirm the *implementation* matches, since fixed windows are the easy thing to accidentally build.

**Fail-open vs fail-closed.** When the counter store is down, `withinBudget` can't answer. Fail-open keeps `SkyCast` serving but drops all protection exactly when the system is fragile; fail-closed returns `Throttled` to everyone and turns a counter outage into a full outage. The right answer depends on whether `SkyCast` fears abuse or downtime more — and the model is silent on it.

**What's the key, really?** `apiKey` is the budget dimension here, but an attacker with no key (or many free keys) routes around it. Real limiters often need a second dimension — IP, account, or endpoint — and a sane default budget for unauthenticated traffic.

## Resilience

The model omits the counter store, the window algorithm, the fail mode, and any feedback to the caller. Harden it by: returning a `Retry-After` hint with `Throttled` so well-behaved clients back off instead of retrying instantly (a retry storm against the limiter is its own DoS); using a token bucket or sliding window over a fixed window to kill the boundary burst; co-locating or sharding the counter store so the limiter check stays sub-millisecond; and choosing the fail mode deliberately per endpoint — fail-open for read-only forecasts, fail-closed for anything costly or sensitive.

## Pairs well with

- **Retry with backoff** — clients that respect `Retry-After` and back off keep the limiter from being hammered by the very traffic it just rejected.
- **Circuit breaker** — if `Forecasts` itself degrades, a breaker sheds load the limiter alone wouldn't catch (the limiter caps *per key*, the breaker reacts to *the service*).
- **Load shedding / priority queues** — when globally overloaded, drop low-priority keys first rather than throttling everyone equally.
- **Bulkhead** — isolate the limiter's counter store so its failure doesn't take the API down with it.
