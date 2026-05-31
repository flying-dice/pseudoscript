# Scatter-Gather

You want car insurance. You could open ten insurer websites, type your details ten times, and squint at ten prices. Or you could ask QuoteMesh once and get back the single best offer. From your seat it is one question and one answer. Behind the screen, QuoteMesh asked everyone.

## The problem

A driver's `Shopper.compare` carries one `Cover` — a car and a person. The answer they want is one number: the cheapest premium. But that number does not live in any one place. It is spread across `InsurerA`, `InsurerB`, and `InsurerC`, each an independent system that prices the same cover differently and knows nothing of the others. No single insurer can answer "what is the best quote?" — that answer only exists once you have asked all of them and compared.

So the work is inherently fan-out: ask many, then reconcile the many answers into one.

## The pattern

`Compare`, the container inside `QuoteMesh`, is where scatter and gather meet. Its `quote` endpoint (`POST /quotes`) does two distinct jobs.

First the **scatter**: it calls `InsurerA.price(cover)`, `InsurerB.price(cover)`, and `InsurerC.price(cover)`. Same `Cover` to every insurer, fanned out across the three independent systems. Each returns its own `Quote` — an insurer name and a premium.

Then the **gather**: `Compare.best(a, b, c)` folds the three `Quote`s into one `Best`, picking the lowest premium. That aggregation is the whole point. The driver asked one question and gets one `Best` back; the `FanOutAndAggregate` feature pins this exactly — "the driver sees one offer, not the individual quotes." The three insurers, the fan-out, the comparison logic — all of it is hidden behind `Compare`, which presents itself to `Shopper` as a single call.

Notice the shape: the requester talks to one coordinator (`Compare`), the coordinator talks to many workers (`InsurerA/B/C`), and the coordinator owns the reduction. Add `InsurerD` and only `Compare` changes; `Shopper` never knows.

## When to use it

Use scatter-gather when one logical answer is a function of many independent sources you can query in parallel — price comparison, search across shards, polling replicas for the freshest value, soliciting bids. It shines when the workers are independent (no ordering between them) and the reduction is simple (min, max, merge, vote).

## When to avoid it

Avoid it when the workers depend on each other's output — that is a pipeline, not a fan-out. Avoid it when one slow worker can hold the whole response hostage and you cannot tolerate the wait. And skip it when a single source already has the answer; fanning out three identical calls to get one number you could have asked for once is just amplified load.

## Trade-offs

You buy completeness — the *best* offer, not *an* offer — and you pay in coordination. `Compare` becomes a focal point every request flows through, latency is governed by the slowest insurer to reply, and each customer request multiplies into N upstream calls. The next doc walks those failure modes in detail.
