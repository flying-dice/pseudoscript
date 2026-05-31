# Event Sourcing

An append-only log is beautiful right up until you need to change the shape of an event, delete one for legal reasons, or fold a stream that's grown to a million entries. The model's `Events` log is immutable by design — and immutability is exactly what makes these cases hard.

## Edge cases & failure modes

- **Event versioning.** `Expense { member, amount }` is the event today. Add a `currency` field next year and `Balances.fold` must still correctly process every old `Expense` that predates it. You can never "migrate" the log — it's immutable — so old and new event shapes coexist forever. You need explicit versioning and upcasting, or `fold` silently mishandles history.
- **Replay cost on long streams.** `Balances.rebuild(member)` replays *the entire* `Events.history`. For a member with a decade of expenses, every rebuild folds thousands of events. Replay is correct but it is not free, and it gets slower for life.
- **Snapshots.** The standard fix: periodically persist a folded balance plus the offset it was computed at, then `rebuild` replays only events *after* the snapshot. Snapshots are an optimisation, never the source of truth — the log still is — but without them, long streams make replay impractical.
- **GDPR / the right to be forgotten.** A `Member` asks to be deleted, but their `Expense` events are in an append-only, never-deleted log. Immutability collides head-on with deletion law. The usual escape is crypto-shredding: store personal fields encrypted per-member and destroy the key, leaving the event structurally intact but unreadable.
- **Eventual consistency of projections.** `Balances` updates on `#[onevent]`, so it lags the `append`. A balance read right after `record` may not include that expense yet.

## Resilience

Treat the `Events` log as the one durable source of truth and replicate it accordingly — everything else (`Balances`) is rebuildable and therefore disposable. Version events from day one; an unversioned event is a future migration you can't perform. Snapshot long streams and store the snapshot offset so replay is bounded. Make `Balances.fold` idempotent and offset-aware so a redelivered `Expense` doesn't double-count. Test `rebuild` continuously — a projection you can't rebuild is a projection you don't really have.

## Pairs well with

- **CQRS** — `Balances` is naturally a read model derived from the `Events` log; the two patterns are made for each other.
- **Outbox** — append to `Events` and publish atomically so no `Expense` is recorded without being observable.
- **Idempotent receiver** — the property that makes `Balances` safe under replay and redelivery.
