# CQRS

The model shows the clean version: a write emits, a projector bumps, the leaderboard serves. The interesting part is the gap between the write landing and the leaderboard showing it.

## Edge cases & failure modes

- **Eventual consistency between write and read.** `WriteModel` records the kudos and emits `KudosGiven`; `Standings` only updates when `Projector` gets around to it. Between those two moments the leaderboard is *wrong* — correct soon, wrong now. Every CQRS system lives with this window; the only question is how long it is and whether anyone notices.
- **Read-your-writes.** A `Teammate` thanks a mate, then immediately opens `GET /leaderboard` and their kudos isn't there yet. To a user this reads as a bug, not a consistency model. You either mask it (optimistically show the write client-side) or accept it — but you must decide deliberately.
- **Projection lag under load.** `Projector` processes `KudosGiven` events one at a time. A burst of writes, or a slow `Standings.bump`, and the projector falls behind. The leaderboard isn't stale by a known bound anymore; it's stale by however deep the backlog is.
- **Rebuilds.** Because `Standings` is *derived*, you can throw it away and rebuild it from the event stream — which is a superpower for fixing bugs or adding a new read shape. But a rebuild replays the whole history through `Projector`, which is slow and, while it runs, the leaderboard may be empty or partial.
- **Two schemas, two migrations.** `WriteModel` and `Standings` evolve separately. Add a field to the write side and the projector and read side must learn about it in the right order, or events arrive that `Projector` can't interpret.

## Resilience

Make `Projector` idempotent so a redelivered `KudosGiven` doesn't double-count — bumps must be safe to replay. Track projection offset/lag as a first-class metric and alert when `Standings` falls behind the write log. Keep the projector's work small and fast so it drains backlogs quickly. For rebuilds, build into a shadow read model and swap atomically so the live `GET /leaderboard` never serves a half-built board. And design the UI to tolerate the consistency window rather than fight it.

## Pairs well with

- **Event Sourcing** — make `WriteModel`'s event stream the source of truth, and `Standings` becomes a pure, rebuildable projection of it. CQRS and event sourcing are the classic pairing.
- **Outbox** — emit `KudosGiven` atomically with the write so an event is never lost between `WriteModel` and `Projector`.
- **Idempotent receiver** — the discipline that lets `Projector` safely reprocess events on redelivery or replay.
