# Sharding

`Router` resolves a code, `Shards` picks the owner, one `Shard` answers. Clean — as long as load is even, the shard set never changes, and no request needs two shards at once. Production violates all three.

## Edge cases & failure modes

- **Hot shards and skew.** Partitioning by `code` assumes codes spread evenly across shards. They rarely do. One viral link gets followed millions of times, and the `Shard` that owns its code runs hot while its peers idle. The whole point — "no single node is the bottleneck" — quietly fails for that one partition. Skew is the default, not the exception.
- **Resharding and rebalancing.** Snip grows and needs more shards. But the mapping from `code` to `Shard` is baked into how `Shards` dispatches; add a shard and a naive hash remaps *most* codes, so `resolve` looks on the wrong shard and links appear to vanish. Migrating data while `Router` keeps serving live traffic is one of the hardest operations a sharded system ever performs.
- **Cross-shard queries.** `Shards.read(code)` is a single-key lookup, which is what sharding is good at. Ask "list every link a user created" and that data is scattered across every `Shard`; the query becomes a scatter-gather over all of them, as slow as the slowest shard.
- **Cross-shard transactions.** Two links on two different shards can't be updated in one atomic step — each `Shard` is "an independent owner." There's no shared transaction across partitions, so multi-shard writes need sagas or two-phase commit, both costly and failure-prone.
- **Choosing the shard key.** The `code` key works because lookups are by code. Pick a key the queries don't use and every read becomes a cross-shard scan. The shard key is the single most consequential — and least reversible — decision here.

## Resilience

Use **consistent hashing** (or an explicit lookup table) so adding a `Shard` remaps a small slice of codes, not all of them, and rebalancing stays incremental. Replicate each `Shard` so losing a partition doesn't lose its whole slice of the keyspace. Watch per-shard load and split or migrate hot partitions before they tip over. Keep the partition map in one authority — Snip already centralises it in `Router`/`Shards` — so routing never disagrees with itself. Design access so the common path is a single-key lookup; push rare cross-shard reads to an async or secondary index rather than the hot path.

## Pairs well with

- **Scatter-gather** — the structured way to answer a query that genuinely must touch every `Shard`.
- **Leader election** — pick a primary per `Shard` replica set so writes have one authoritative owner.
- **CQRS** — maintain a separate read model for the cross-shard queries that sharding makes expensive.
