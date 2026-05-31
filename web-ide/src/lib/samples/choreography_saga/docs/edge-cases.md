# Edge Cases & Resilience

The Marketplace flow looks effortless: `Orders` emits, `Payments` reacts, `Shipping` reacts. But the flow lives in the *gaps between* services, and that is exactly where things break.

## Edge cases & failure modes

- **Lost events.** `Orders` emits `OrderPlaced` on the `Bus`, but `Payments.onPlaced` never sees it — a bus hiccup, a consumer crash before the handler ran. There is no orchestrator polling for "did payment happen?", so a dropped event silently strands the order forever.
- **Duplicated events.** Delivery is usually at-least-once, so `Payments.onPlaced` may fire twice for one `OrderPlaced`. Without protection that's a double charge; `Shipping.onPaid` firing twice ships twice.
- **Compensation that itself fails.** If `Shipping` can't fulfil after `Payments` already emitted `Paid`, *someone* must emit a refund-style compensating event. But there's no orchestrator to notice and trigger it — the compensation is just another reaction that can also be lost or fail.
- **Non-compensatable steps.** Once `Shipping` emits `Shipped` and a parcel is on a van, no event un-ships it. Irreversible steps must sit at the end of the chain.
- **Lack of isolation / dirty reads.** Between `OrderPlaced` and `Shipped` the order is mid-flight: paid but not shipped, or placed but not paid. Any reader sees a partial, possibly-doomed state — there's no transaction isolating the in-progress saga.
- **Observability — the missing central view.** This is choreography's signature weakness. No component knows the whole journey, so "where is order #42, and why is it stuck?" has no single place to answer it. You reconstruct the saga by correlating `OrderPlaced` / `Paid` / `Shipped` across three services.

## Resilience

- **Idempotent steps.** Make `onPlaced` and `onPaid` idempotent, keyed by `order`. A redelivered `OrderPlaced` then charges once; a redelivered `Paid` ships once. This is what makes at-least-once delivery survivable.
- **Correlation IDs + a tracing view.** Stamp every event with the `order` id and build a read-model that subscribes to all three events. That reconstructed timeline is the central view choreography otherwise lacks — your answer to "where is order #42?".
- **Timeouts / saga timeouts.** Since no orchestrator watches for stalls, add a watchdog: if `Paid` hasn't followed `OrderPlaced` within a deadline, flag or compensate. Without it, lost events stay lost.
- **Dead-letter queues.** A handler that keeps failing on an event should dead-letter it rather than block the bus or loop forever — then a human or job can replay it.
- **Beware cyclic event dependencies.** When service A reacts to B's event and B reacts to A's, you've built a loop that's invisible until it runs. Map who-emits-what and who-listens-to-what, and keep the event graph acyclic.

## Pairs well with

- **Transactional outbox** — `Orders` must persist the order and the `OrderPlaced` event in one local commit, so a crash can't leave a saved order that never emitted (the classic lost-first-event bug).
- **Idempotent receiver** — the deduplicating side of `onPlaced` / `onPaid`, dropping repeats by `order` id so at-least-once delivery is safe.
- **Retry with backoff** — let a reactor retry a transient failure before dead-lettering, so a flaky downstream doesn't strand the saga.
- **Correlation / tracing** — by `order` id, the only practical way to recover the end-to-end view in a system with no orchestrator.
