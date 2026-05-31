# Sharding

Billions of short links won't fit on one database. Not "won't fit comfortably" — won't fit. Past a certain size you stop asking how to make one box bigger and start asking how to spread the data across many. Sharding is that answer, made deliberate.

## The problem

**Snip** is a URL shortener storing billions of links — `Link { code, target }`. A **Surfer** follows a short code; somewhere a row must turn that code back into its target. At this scale, one database is impossible: no single machine has the disk, the IOPS, or the memory to hold and serve the whole keyspace. Vertical scaling runs out — there's no bigger box to buy. And even before the ceiling, one database means one bottleneck: every read and every write funnels through the same node, and that node's limits become the product's limits.

You need the data spread across many independent databases. But the moment you do that, a new question appears: given a code, *which* database has it?

## The pattern

Sharding partitions the data by a key and routes every operation to the partition that owns it. The art is in two places: choosing the key, and keeping the routing in exactly one place.

Trace Snip. A `Surfer.follow(code)` calls `Snip::Router.resolve`. `Router` is "the only part that knows the partitioning scheme" — it exposes `shorten` (`POST /links`) and `resolve` (`GET /{code}`), and it owns the decision of where a code lives. It delegates to `Shards`: `Router.shorten` calls `Shards.write(link)`, `Router.resolve` calls `Shards.read(code)`.

`Shards` is the shard set — it maps a code to its owning shard and dispatches there: `write` calls `Shard.put`, `read` calls `Shard.get`. And `Shard` is one partition: a `component` that is "an independent owner of the codes that map to it," knowing only `put` and `get`. Each `Shard` is a complete, self-contained database for its slice of the keyspace.

The shard key here is the short `code`. A function of the code (a hash, typically) decides the owning shard, so reads and writes for the same code always land on the same partition. The `PartitionByCode` feature states the guarantee: each request goes "to the one shard that owns that code," "other shards serve unrelated codes in parallel," and "no single node is the bottleneck."

## When to use it

When a dataset or its throughput exceeds what one node can hold or serve, and the access pattern keys cleanly on something you can partition by. It shines for huge, evenly-distributed keyspaces where requests touch one key at a time.

## When to avoid it

When one node still comfortably fits the data — sharding adds a routing layer and operational weight you don't need. Avoid it too when queries routinely span many keys; cross-shard work erodes the whole benefit.

## Trade-offs

You gain near-linear horizontal scale and parallelism across shards. You pay with a routing layer, the difficulty of multi-shard queries and transactions, and the operational pain of rebalancing when shards grow uneven.
