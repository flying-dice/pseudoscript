# The Pattern

A shopper clicks "place order." Two things must now be true: the order is saved, and fulfilment has been told. The trap is that the database and the event bus are different systems with no shared transaction — so one can succeed while the other fails. Transactional Outbox closes that gap.

## The problem

Cartwheel is an online store. `Checkout.place` must record the order *and* announce it to fulfilment. The obvious code writes the order to the database, then publishes an `OrderPlaced` event to the bus. But these are two separate commits across two systems. If the process crashes between them, you get the dual-write problem in its purest form: the order exists but no one was told (a paid order never ships), or — if you publish first — fulfilment ships an order the database never recorded. There is no ordering of these two writes that is safe, because nothing makes them atomic.

## The pattern

Cartwheel makes the two writes one write. The `OrderStore` keeps the orders *and* an outbox table in the same database. `Checkout.place` calls `OrderStore.commit(order, OrderPlaced from { order })` — and because both rows live in one database, a single transaction covers both. Either the order and its `OrderPlaced` outbox row are persisted together, or neither is. The dual write is gone; there is only one commit.

Publishing is now a separate, recoverable step. The `Relay` container runs on a `#[schedule]`: its `drain` calls `OrderStore.pending()` for unpublished events oldest-first, and for each one it calls `Bus.publish(event)` then `OrderStore.markSent(event)`. The `Bus` is an external system fulfilment listens on. Crucially, the relay retries until the bus acknowledges — so a crash *after* commit but *before* publish loses nothing. On restart, the event is still sitting in the outbox marked unsent, and the next `drain` picks it up.

The `NoLostOrders` feature names the guarantee: given an order written in one transaction with its outbox event, when the relay later drains the outbox, the `OrderPlaced` event reaches the bus at least once — *but a crash between commit and publish loses no order and invents none*. Every persisted order is eventually announced, and only persisted orders are.

## When to use it

Use it whenever a state change and an event about that change must both happen, across a database and a message system that share no transaction. It is the backbone of reliable event-driven systems and the standard fix for dual writes in microservices.

## When to avoid it

Skip it when there is no event to publish — a plain database write needs no outbox. Skip it when your messaging and storage already share a transaction. And weigh it against the latency it adds: the relay polls, so events arrive *after* a poll cycle, not instantly.

## Trade-offs

You trade immediate publishing and a little simplicity for a rock-solid guarantee. The outbox adds a table, a relay to operate, and polling latency. In return you get atomicity without distributed transactions, and at-least-once delivery that survives any crash. The bill comes due downstream: at-least-once means duplicates, so consumers must be idempotent.
