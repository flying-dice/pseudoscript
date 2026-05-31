# The Pattern

A ship doesn't sink because one compartment floods — watertight bulkheads keep the breach contained. **Wanderlust**, a travel meta-search, applies the same idea to a software dependency that stalls.

## The problem

`Wanderlust` runs a single search that fans out to three providers: flights, hotels, and cars. A `Traveller` searches a destination and expects all three back.

The trap is shared resources. If every provider call draws from one connection pool (or one thread pool), a single slow provider quietly poisons the whole search. Say the hotel provider stalls. Hotel requests don't error — they *hang*, holding their connections open. With one shared pool, those hung hotel calls consume every slot, and now flight and car requests — which would have returned instantly — queue up with nothing to run on. One stalled dependency has taken down a search that was 2/3 healthy.

## The pattern

A bulkhead partitions resources so a failure in one partition can't drain the others. In the model, `Wanderlust`'s `Meta` container does **no work itself** — it's a pure dispatcher. Each of its endpoints routes to a *separate* pool container:

- `Meta.flights(q)` → `FlightsPool.query(q)`
- `Meta.hotels(q)` → `HotelsPool.query(q)`
- `Meta.cars(q)` → `CarsPool.query(q)`

`FlightsPool`, `HotelsPool`, and `CarsPool` are three distinct containers, each its own bounded compartment. When a pool is exhausted, its `query` returns `Err(PoolFull { lane })` — note the `lane` field names *which* compartment filled, so the failure is attributable.

Now replay the hotel stall. `HotelsPool` fills with hung requests and starts returning `PoolFull { lane: "hotels" }`. But `FlightsPool` and `CarsPool` are untouched — different containers, different slots — so `Meta.flights` and `Meta.cars` keep returning `Results` normally. The `OnePoolFailsAlone` feature pins exactly this: *"the flight and car lanes return results on their own pools, but only the hotel lane is rejected as pool-full."* The breach is contained to one compartment.

## When to use it

Use a bulkhead whenever one process serves several independent downstreams or workloads of differing reliability and you can't let the weakest drag down the rest. Multi-provider aggregation (like `Wanderlust`), a shared API gateway fronting many backends, or separating premium traffic from best-effort traffic are textbook cases.

## When to avoid it

Avoid it when there's only one downstream — there's nothing to isolate *from*. Avoid it when your workloads are so spiky that statically partitioned pools waste capacity most of the time; a single elastic pool may serve you better. And don't reach for it when the real fix is making the slow dependency fast or removing it.

## Trade-offs

Isolation costs utilization. Three fixed pools mean the hotel lane can be starving while the flight pool sits half-idle — capacity you can't lend across the wall, by design. Sizing becomes the hard part: pools too small reject healthy traffic, too large and the bulkhead stops protecting anything. And `N` lanes is `N` things to monitor and tune. You're buying fault isolation with efficiency and operational overhead.
