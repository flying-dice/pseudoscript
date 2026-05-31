# Edge Cases & Resilience

`NoLostOrders` is a strong promise — never lose an order, never invent one. The way the `Relay` keeps it has sharp corners.

## Edge cases & failure modes

**The dual-write problem it solves.** Worth naming precisely, because it is the whole reason the pattern exists. Two independent writes — order to the database, `OrderPlaced` to the bus — cannot be made atomic across two systems. A crash between them yields a ghost order (saved, unannounced) or a phantom shipment (announced, unsaved). The outbox dissolves this by folding the event into the *same* transaction as the order, leaving exactly one write to fail or succeed.

**Relay duplicates.** Look at `drain`: it calls `Bus.publish(event)` then `OrderStore.markSent(event)`. If the process crashes *between* those two lines, the event was published but never marked sent. The next `drain` republishes it. This is unavoidable and by design — it is why the guarantee is "at least once," not "exactly once." Downstream consumers on the `Bus` must be idempotent, keying on the order id so a redelivered `OrderPlaced` is a no-op.

**Polling vs CDC.** The model polls: `Relay` wakes on a schedule and calls `pending()`. Polling is simple and easy to reason about, but it adds latency (events wait for the next cycle) and load (queries that often find nothing). The alternative is Change Data Capture — tailing the database's transaction log to publish outbox rows the instant they commit. CDC cuts latency and load but couples you to the database's log format and adds operational weight.

**Ordering.** `pending()` returns events oldest-first, and a single relay preserves that order. Run *two* relays for throughput and ordering can break — both may grab overlapping batches. If consumers depend on per-order ordering, either keep one relay per partition or have consumers tolerate reordering via version numbers.

## Resilience

The relay must be safely re-runnable: `markSent` is the idempotency boundary on the publish side, and consumers' own idempotency covers the gap between publish and mark. Cap retries with backoff so a wedged `Bus` does not spin hot, and dead-letter events that never acknowledge. Monitor outbox lag — the age of the oldest `pending()` row — as the signal that the relay has fallen behind or stalled.

## Pairs well with

**Publish/Subscribe** is the natural sink: the relay publishes `OrderPlaced` to a broker that fans it out to fulfilment, billing, and analytics — reliable production feeding flexible consumption.

**Competing Consumers** drain the bus: a worker pool processes published `OrderPlaced` events in parallel, idempotently absorbing the relay's duplicates.

**Claim Check** keeps outbox rows small when an event carries a large payload — store the payload, put a reference in the outbox.
