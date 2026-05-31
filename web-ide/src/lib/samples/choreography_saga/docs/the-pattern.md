# The Saga Pattern (Choreographed)

A buyer clicks **Buy** on the **Marketplace**. Somewhere downstream a payment is taken and a parcel is shipped — but if you go looking for the function that *runs* that sequence, you won't find one. No component calls "take payment, then ship". Instead, each service watches for an event, does its one job, and announces what it did. The end-to-end order flow is not written down anywhere; it *emerges* from services reacting to each other.

This is a choreographed saga.

## The problem

Placing an order spans three concerns — recording the order, taking payment, shipping the goods. In a single database you'd wrap them in one transaction and commit atomically. Across services, that commit doesn't exist. `Orders`, `Payments`, and `Shipping` are separate containers with separate state. You need them to advance together — order leads to payment leads to shipment — without a shared lock, and ideally without any one of them having to *know about* the others.

A saga buys this back with reversible steps and forward progress. Choreography goes further: it removes the conductor entirely.

## The pattern

Every step listens for the previous step's event and publishes its own on the shared `Bus`:

1. `Orders.place(order)` records the order and emits `OrderPlaced` (`Bus.orderPlaced(...)`). It does **not** call payments.
2. `Payments.onPlaced` is wired with `#[onevent(OrderPlaced)]`. It reacts, takes payment, and emits `Paid` (`Bus.paid(...)`). It has never heard of shipping.
3. `Shipping.onPaid` is wired with `#[onevent(Paid)]`. It reacts, ships, and emits `Shipped` (`Bus.shipped(...)`).

No component holds the script. `Orders` knows only "I emit `OrderPlaced`". `Payments` knows only "when `OrderPlaced`, emit `Paid`". The journey is the *sum* of these local reactions. The `FlowByReaction` feature pins exactly this: payments reacts to the order-placed event, takes payment, emits paid; shipping reacts to paid and ships — *but no central orchestrator drives the sequence*.

## Orchestration vs choreography

The sibling example, **Saga (Orchestrated)**, puts one component — a `Planner` — in charge of the whole journey. Read its `book` method and you see every step and every rollback in one place.

Choreography deletes that component. The trade is fundamental. Orchestration centralises control: the flow is easy to read, easy to change, and easy to observe, but the orchestrator becomes a hub coupled to every step. Choreography distributes control: `Orders`, `Payments`, and `Shipping` stay decoupled and independently deployable — you can add a `Notifications` service that also reacts to `Paid` without touching anyone — but the price is that *no single place describes the whole flow*. To understand the order journey, you must trace events across services. Same eventual-consistency guarantee; opposite answer to "who's in charge?".

## When to use it

- Services should stay loosely coupled and independently deployable, reacting to events rather than calling each other.
- The flow is mostly linear or fan-out, and new reactors should be addable without editing existing services.
- You already have a reliable event bus and an event-driven culture.

## When to avoid it

- The flow has complex branching, conditional compensation, or strict ordering that's far clearer in one orchestrated place.
- You need a single, authoritative view of where each order is — choreography scatters that.
- The team can't yet reason about emergent, distributed control flow.

## Trade-offs

You gain decoupling and extensibility: each service does one thing and announces it, and new listeners cost nothing to the old ones. You lose the bird's-eye view — the saga lives in the *wiring between* services, not in any one of them. Debugging means following events, cyclic event dependencies are easy to create by accident, and there is no `Planner` to ask "where is order #42?". Choreography is elegant precisely because nobody is in charge — which is also exactly why it's hard to see.
