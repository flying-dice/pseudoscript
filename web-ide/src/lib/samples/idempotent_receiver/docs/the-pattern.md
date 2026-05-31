# The Idempotent Receiver

A customer pays. The card processor fires a "charge succeeded" webhook at PayLoop, PayLoop credits the order, everyone is happy. Then, three seconds later, the same webhook arrives again. And maybe a third time. Does the customer get credited twice? Three times?

They should not — and PayLoop makes sure of it.

## The problem

PayLoop receives payment webhooks from a card processor it does not control. The processor guarantees **at-least-once** delivery: it would rather send a webhook twice than risk losing it. That is the right call for a sender — networks drop packets, acknowledgements time out, retries are how you survive that. But it pushes a problem onto the receiver. The same `Webhook`, carrying the same `eventId`, can land more than once, and every landing looks like a fresh, legitimate event.

If "apply this payment" runs twice, the order is credited twice. The fix is not to make the processor send less. You cannot; it is not yours. The fix is to make a *repeat* arrival do nothing.

## The pattern

PayLoop splits the work across two containers. `Receiver` is the front door — it owns the `receive` endpoint wired to `POST /webhooks/payments`. `SeenLog` is the memory: it records which `eventId`s have already been processed.

Walk the flow in `Receiver.receive`. Before doing any real work it asks `SeenLog.check(hook.eventId)`. That returns `Result<void, Unseen>` — `Ok` if this id was processed before, `Err(Unseen)` the very first time. Only when the check comes back as an error (`seen.isErr`) does `Receiver` do the irreversible thing: `self.credit(hook)`, then `SeenLog.mark(hook.eventId)` so the next arrival is recognised. The `credit` step is reachable exactly once per event id. A duplicate falls straight through the `if` and returns having done nothing — a no-op, exactly as the `DuplicatesAreNoOps` feature specifies.

The key move: the **`eventId` is the identity of the effect**, not of the message. Two messages with the same id are the same event, however many times the wire delivers them.

## When to use it

Reach for an idempotent receiver whenever a producer you don't control retries, whenever you consume from an at-least-once queue (most of them), and whenever the action behind the message is not naturally repeatable — crediting money, sending an email, shipping an order.

## When to avoid it

If the operation is already idempotent — "set balance to X", "mark as paid" — duplicates are harmless and a `SeenLog` is dead weight. Skip it when there is no stable, sender-provided id to dedupe on; without `eventId` you have nothing to recognise.

## Trade-offs

You trade storage and a lookup for safety. Every processed id lives in `SeenLog`, and that log grows forever unless you expire it. "Exactly-once" is the promise you make to the customer; underneath it is honestly at-least-once delivery plus a dedupe check. That is the whole trick — and it is enough.
