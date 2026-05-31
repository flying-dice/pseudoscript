# The Pattern

A public API is a shared resource, and shared resources get abused. **SkyCast**, a public weather API, shows how a rate limiter keeps one greedy caller from ruining the forecast for everyone else.

## The problem

`SkyCast` hands out developer keys and serves forecasts. A `Developer` calls `lookup` with their key and a city; behind it, the `Forecasts` system does the real work of producing a `Forecast`.

The danger is that callers are not equal. One developer ships a bug — a tight loop, a runaway cron — and starts hammering `SkyCast` thousands of times a second. Without a limit, that flood reaches `Forecasts`, saturates it, and now *every* developer's calls slow down or fail. A single misbehaving key has degraded a shared service. The forecast service can't tell intentional abuse from an accident; either way it drowns.

## The pattern

A rate limiter admits traffic only up to a budget and sheds the excess *cheaply*, before it touches the expensive work. In the model, the `Limiter` container fronts `SkyCast` and gates every call in `lookup`:

1. `lookup` first calls `self.withinBudget(call.apiKey)`. This checks the key's per-minute budget — a token bucket, as the doc-comment notes.
2. If the budget is spent, `withinBudget` returns `Err`, and `lookup` returns `Err(Throttled { apiKey })` *immediately*. The request never reaches `Forecasts`.
3. If there's budget left, `lookup` forwards to `Forecasts.lookup(call.city)` and returns the `Forecast`.

The key detail is *where* the rejection happens. `Throttled` is returned before the forecast service is ever called, so shedding load is cheap — the limiter does a counter check, not a forecast computation. And the budget is keyed *per `apiKey`*, so the flooding key burns through its own bucket while every other key keeps its full allowance. The `ExcessIsShed` feature pins exactly this: *"the call is rejected as throttled before reaching the forecast service, but other keys within budget are unaffected."* The blast radius is one key.

## When to use it

Reach for a rate limiter on any shared, public, or metered surface: public APIs, login endpoints (to blunt brute force), expensive operations, anything where one caller's volume can hurt the rest. It's also how you enforce billing tiers — different budgets per `apiKey`.

## When to avoid it

Skip it on purely internal, trusted, single-consumer paths where there's no contention to manage. Be wary of limiting work that's already cheap and idempotent — the limiter check may cost more than the call. And don't use a rate limiter to paper over a service that simply can't handle its legitimate load; that's a capacity problem.

## Trade-offs

A limiter trades some legitimate throughput for protection: set the budget too low and you throttle honest callers during a normal spike. It adds shared, fast-moving state (the per-key counters) that must be consistent and low-latency — itself a dependency that can fail. And it forces a hard policy choice when *that* state is unavailable: fail-open (let everything through, lose protection) or fail-closed (reject everything, lose availability). Neither is free.
