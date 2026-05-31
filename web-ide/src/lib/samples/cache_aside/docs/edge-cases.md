# Cache-Aside

`Profiles.get` reads clean: check cache, miss, load, fill, return. Every hard problem in caching lives in the gaps that one method doesn't show — what happens when the data changes, when the key expires, and when everyone misses at once.

## Edge cases & failure modes

- **Stale reads.** `Profiles.get` happily returns whatever `Cache.read` holds. When a profile is updated in `Database`, the cached copy is now wrong, and `get` keeps serving it until the entry expires or is invalidated. The model has no write path back to the cache — so by default Facecard serves stale handles after a rename.
- **Invalidation is the app's job, and it's missing.** Nothing in the model invalidates. A real Facecard must, on every profile update, either `Cache.write` the new value or delete the key. Get the order wrong — delete then write, or write then read a concurrent old load — and you re-cache stale data. This is the dual-write problem: two stores, no shared transaction.
- **Thundering herd on a cold or expired key.** A popular profile's entry expires. Now every concurrent `Visitor` misses at the same instant, and all of them run `Database.load(id)` for the *same* `id` simultaneously. The cache was supposed to shield the database; at the expiry moment it does the opposite.
- **Cache stampede on restart.** Cold cache after a deploy or `Cache` failure means *every* read is a miss. The full read storm the cache exists to absorb hits `Database` all at once.
- **Cache unavailability.** If `Cache.read` itself fails, `get` must degrade to `Database.load` rather than error — the cache is an optimisation, not a dependency.

## Resilience

Tackle the herd by collapsing duplicate misses: a per-key lock or single-flight so only one loader runs `Database.load(id)` while the rest wait for the fill. Soften expiry with jittered TTLs so keys don't all die at the same second, and consider serve-stale-while-revalidate to keep answering during a refresh. Pick an invalidation discipline deliberately — write-through, or delete-on-write — and accept that some staleness window remains. Treat `Cache` as best-effort: on its failure, fall through to `Database` instead of failing the request. Warm hot keys proactively after a cold start so the first wave of `Visitor`s doesn't become a stampede.

## Pairs well with

- **Circuit breaker** — wrap `Database.load` so a stampede against a struggling database fails fast instead of deepening the outage.
- **Rate limiter** — cap the miss traffic reaching `Database` so a cold cache can't overwhelm it.
- **CQRS** — a denormalised read model is, in effect, a durable cache you own and invalidate explicitly.
