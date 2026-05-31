# Edge Cases & Resilience

The happy path is easy: a duplicate webhook hits `SeenLog.check`, comes back `Ok`, and `Receiver.receive` does nothing. The interesting parts of an idempotent receiver are at the edges.

## Edge cases & failure modes

**The race between two concurrent duplicates.** The processor can deliver the same `eventId` twice so fast that both copies reach `Receiver.receive` before either has called `SeenLog.mark`. Both call `check`, both get `Err(Unseen)`, both proceed to `credit` — a double-credit, the exact bug the pattern exists to prevent. The `check`-then-`mark` sequence must be atomic, or `mark` must be the thing that *enforces* uniqueness (a unique constraint on `eventId` that the second writer trips on) rather than a check done earlier and trusted.

**When to forget.** `SeenLog` cannot remember every id forever. Give entries a TTL and you reintroduce the bug: if the processor's retry window outlives the TTL, a late duplicate arrives after its id has been forgotten and is treated as new. The dedupe key's lifetime must exceed the sender's maximum retry horizon.

**Storage growth of the seen-log.** Every processed `eventId` is a row that never naturally goes away. At webhook volume that is unbounded growth. TTL-based expiry bounds it, but only as far as the forgetting problem above allows — the floor on retention is set by the producer, not by your disk.

**At-least-once wearing an exactly-once mask.** It is worth saying plainly: the processor still delivers at-least-once. `SeenLog` does not make delivery exactly-once; it makes the *effect* exactly-once. If `credit` succeeds but `mark` fails, the next duplicate will credit again — so the ordering and atomicity of those two steps is the whole guarantee.

## Resilience

The receiver tolerates duplicates by design, which means it tolerates retries, which means upstream can be aggressive about retrying without harm. Make the operation behind `credit` and the `mark` write commit together; if they can't share a transaction, mark first and make `credit` itself idempotent, or accept that a crash between them yields a missed credit rather than a double one — choose the failure you can live with. Keep `SeenLog` available: if it is down, `check` cannot answer, and you must fail closed (reject the webhook, let the processor retry) rather than fail open into double-processing.

## Pairs well with

`SeenLog` is the natural partner of a **transactional outbox** on the sending side and of an **at-least-once message queue** as the transport — the queue retries freely because the receiver is safe. It complements the **Saga** pattern, where each step must survive redelivery, and **leader_election**, where the single fired job may still be delivered more than once downstream and needs a receiver that shrugs off the repeat.
