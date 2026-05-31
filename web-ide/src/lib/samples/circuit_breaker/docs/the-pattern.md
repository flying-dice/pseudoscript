# The Pattern

A currency widget should never hang for ten seconds because a third-party feed is having a bad day. **RateWatch** shows you how a circuit breaker turns a slow, failing dependency into a fast, graceful fallback.

## The problem

`RateWatch` is a currency-converter widget. A `Trader` clicks "convert" and expects a rate, and `RateWatch` gets that rate from `FxFeed`, a third-party FX provider. `FxFeed` is fast when healthy but flaky under load — its `quote(pair)` call can return a `Timeout`.

The naive design forwards every click straight to `FxFeed`. When the feed starts timing out, every request waits the full timeout before failing. Worse, all that retried traffic piles onto a provider that is already struggling, so it never gets a chance to recover. One sick dependency drags the whole widget down with it.

## The pattern

A circuit breaker is a stateful guard that sits between caller and dependency and *remembers* recent failures. In the model, that guard is the `Breaker` container, and every call to the feed goes through its `rate(pair)` method:

1. `rate` first asks `self.isOpen()`. If the breaker is **open**, it returns `Err(FeedDown)` immediately — no call to `FxFeed` at all. The widget catches that and shows the last-known rate. This is the "fail fast" move: a degraded answer in microseconds beats a timeout in seconds.
2. If the breaker is closed, `rate` calls `FxFeed.quote(pair)`. On success it returns `Ok(Rate)`. On failure it calls `self.record(e)`, which counts the failure toward the trip threshold and returns the `FeedDown` marker.

Enough failures and the breaker trips open. After a cool-down it goes **half-open**: a single trial call probes whether `FxFeed` has recovered. The `TripsOpenThenRecovers` feature pins exactly this — *"only a successful trial closes the breaker again."* One probe succeeds, the breaker closes, normal service resumes; the probe fails, it re-opens and waits out another cool-down. The system never floods a recovering provider.

## When to use it

Reach for a circuit breaker when you call a remote dependency that can fail *slowly* — network services, third-party APIs, anything where a timeout is the failure mode. It shines when you have a sane fallback (a cached `Rate`, a default, a queued retry) and when hammering a struggling dependency makes things worse rather than better.

## When to avoid it

Skip it for in-process or sub-millisecond calls — the bookkeeping costs more than it saves. Skip it when there is no acceptable fallback and a stale answer is worse than an error. And don't add one where simple bounded retries already suffice; a breaker is for *systemic* failure, not the occasional blip.

## Trade-offs

A breaker trades freshness for availability: while open, `RateWatch` knowingly serves a stale rate. It adds shared state (the failure count, the open/closed status) that has to be tuned — trip threshold, cool-down duration — and tuned wrong it either trips too eagerly or never protects you. And it hides the real error behind `FeedDown`, so observability matters: you want to *see* the breaker open, not discover it from confused users.
