# Edge Cases & Resilience

The `FanOutToSubscribers` feature promises clean independent delivery. Production rarely is that clean — here is where it frays.

## Edge cases & failure modes

**At-least-once vs exactly-once.** `Broker.publish` calls each subscriber, but what happens when `Alerts.notify` throws after `SearchIndex.update` already succeeded? If the broker retries the whole publish, search gets the `PriceChanged` event twice. Most real brokers guarantee *at-least-once* delivery, not exactly-once, so subscribers must tolerate duplicates — typically by treating a `(sku, price)` pair idempotently rather than blindly applying a delta.

**Slow and dead subscribers.** The model shows `publish` calling `notify`, `update`, and `record` in sequence. If `Analytics.record` hangs, does the shopper's alert wait behind it? A synchronous broker couples the slowest subscriber to all the others — the very coupling the pattern set out to remove. A dead subscriber is worse: events pile up with nowhere to go.

**Ordering.** Two reprices of the same `sku` in quick succession can arrive at a subscriber out of order, leaving the search index showing the older price. Pub/sub gives no global ordering for free; if a subscriber needs it, the events must carry a version or timestamp and the subscriber must reject stale ones.

**Fan-out backpressure.** One publish becomes three deliveries — multiply that across a flash sale repricing thousands of SKUs and the broker's outbound volume explodes. Fast publishers can overwhelm slow subscribers, growing unbounded queues behind each one.

## Resilience

Make the broker asynchronous: `publish` should hand the event to a per-subscriber queue and return, so a slow `Analytics` never blocks `Alerts`. Give each subscriber its own retry policy and a dead-letter destination for events it repeatedly fails to process, so one poison event does not stall the stream. Make subscribers idempotent so at-least-once redelivery is safe. Apply backpressure or buffering per subscriber so a flash-sale spike degrades gracefully instead of toppling the broker.

## Pairs well with

**Competing Consumers** sits naturally behind each subscriber: fan the `PriceChanged` event out to per-subscriber queues, then let a pool of workers drain each — independent scaling per reaction.

**Transactional Outbox** solves the publish-side gap this model glosses over: if `Catalog` updates its own database and then publishes, the Outbox makes those two steps atomic so a reprice can never be persisted yet go unannounced.

**Claim Check** helps when the event payload is large — publish a small reference through the broker and let each subscriber fetch the heavy data, keeping fan-out cheap.
