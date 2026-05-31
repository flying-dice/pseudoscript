# The Pattern

A shopper waiting for a price to drop should not care how many systems react when it does — and the system that changes the price should not care either. Publish/Subscribe is the discipline that keeps it that way.

## The problem

PriceDrop is a price-watch platform. When a retailer reprices an item, three things must happen: watchers get an email, the search index refreshes, and analytics records the move. The naive design wires the catalogue directly to all three — `Catalog` calls `Alerts`, then `SearchIndex`, then `Analytics`. Now the catalogue knows about every consumer. Add a fourth reaction (a recommendation engine, say) and you re-open and redeploy the catalogue. The thing that produces the event is coupled to everything that consumes it, and that coupling only grows.

## The pattern

PriceDrop breaks the link with a `Broker`. The `Catalog` container does exactly one thing on a reprice: `onRepriced` calls `Broker.publish(event)` with a `PriceChanged` record. It names no subscriber. The `Broker` is the single place that knows the subscriber list — its `publish` fans the one event out to `Alerts.notify`, `SearchIndex.update`, and `Analytics.record`, each reacting independently.

Notice the direction of knowledge. Subscribers register themselves: the `Shopper` person calls `Alerts.subscribe(sku)` to start watching. Subscription flows *toward* the broker; publishing flows *away* from it. The publisher and the subscribers never reference each other — they only share the `PriceChanged` event shape and a broker between them.

The `FanOutToSubscribers` feature pins the guarantee: given alerts, search, and analytics all subscribed at the broker, when the catalogue publishes one event, the broker delivers it to each subscriber independently — *but the catalogue knows nothing about who subscribes*. That last clause is the whole point. To add a consumer you touch only the broker's subscriber list; the publisher is untouched.

## When to use it

Reach for pub/sub when one event has many independent reactions, when the set of reactions changes over time, and when consumers can act on their own without the producer waiting for them. It shines for cross-cutting concerns — search indexing, analytics, notifications — that should never be wired into core business logic.

## When to avoid it

Skip it when there is exactly one consumer and there always will be: a broker is just overhead and indirection then. Avoid it when the producer needs a result back — pub/sub is fire-and-forget, so request/response or a direct call is clearer. And be wary when every subscriber must succeed atomically with the publish; the broker decouples them precisely so they can fail independently.

## Trade-offs

You trade compile-time clarity for runtime flexibility. With `Catalog` calling subscribers directly you can read the fan-out in one file; with the `Broker` you must inspect the live subscriber list to know who actually reacts. You gain effortless extension and isolation — a broken `Analytics` consumer cannot stall an `Alerts` email — at the cost of a new moving part to operate, monitor, and reason about.
