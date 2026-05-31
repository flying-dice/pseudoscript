# Edge Cases & Resilience

The happy path of Voyagr's `Planner.book` is short. The interesting engineering is everything that happens when a step — or a *compensation* — goes wrong.

## Edge cases & failure modes

- **Compensation that itself fails.** When `Cars.reserve` fails, `Planner` calls `Hotels.cancel(trip)` and `Flights.cancel(trip)`. But what if `Hotels.cancel` fails? The forward path returns a tidy `Result`; the rollback path doesn't. A failed compensation leaves a *stuck saga* — a flight booked that nobody is paying for. This is the single most dangerous case, because the system is now inconsistent and the saga has run out of moves.
- **Non-compensatable steps.** `cancel` assumes every `reserve` can be undone. Some can't: a non-refundable fare, a supplier with no cancellation API. If a step has no business-level undo, it must move *last* in `Planner`, so nothing after it can force a rollback it can't perform.
- **Partial failure & retries.** A `reserve` may not return cleanly — it may time out with the booking actually made on the supplier side. Blindly retrying double-books; blindly compensating cancels a reservation that never existed. The saga needs to know the true state before it acts.
- **Lack of isolation / dirty reads.** Between the first `Flights.reserve` and the final `Ok`, the trip is half-built. Anyone reading supplier inventory mid-saga sees a reservation that may be cancelled seconds later — there is no transaction isolating the in-flight saga from other readers.
- **Observability.** When a booking is "stuck", an operator needs to see *which* step `Planner` reached and *which* compensations ran. Without an explicit record, the saga's progress is invisible — buried in logs across three suppliers.

## Resilience

- **Idempotent steps.** Make `reserve` and `cancel` idempotent, keyed by `Trip.id`. Then a retried reserve is a no-op if it already succeeded, and a cancel is safe to send twice. This is what makes "retry on timeout" safe.
- **A saga log / state machine.** Persist each transition of `Planner.book` — *flight reserved*, *hotel reserved*, *compensating car failure* — before performing it. The log is the source of truth: on crash or restart, the saga resumes from the last recorded step instead of starting blind.
- **Timeouts.** Give every `reserve` a deadline. A supplier that never answers must surface as a definite failure the saga can act on, not an indefinite hang.
- **Dead-letter for stuck sagas.** When compensation itself fails and the saga is out of automated moves, route it to a dead-letter queue for human or scheduled retry. A failed rollback must never be swallowed — it must become a visible, actionable item.

## Pairs well with

- **Transactional outbox** — record "must cancel hotel" in the same commit that records the failure, so a crash between the two can't drop the compensation.
- **Idempotent receiver** — the receiving side of idempotent `reserve`/`cancel`, deduplicating by `Trip.id` so retries are free.
- **Retry with backoff** — for transient supplier failures, before the saga concludes a step has truly failed and triggers compensation.
- **State machine** — model `Planner` explicitly as states and transitions so the saga's position is always inspectable, and recovery is just "resume from the recorded state".
