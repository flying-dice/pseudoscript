# Edge Cases & Resilience

`OneJobOneWorker` promises each upload is resized exactly once. The hard part is keeping that promise when a worker dies mid-job.

## Edge cases & failure modes

**Visibility timeout.** When a `Resizer` calls `Queue.dequeue`, the job must not vanish from the queue — it must be *hidden* for a window. If the worker crashes resizing a hundred-megapixel image, the job has to reappear so another worker can retry. Set that visibility window too short and a slow-but-healthy resize gets handed to a second worker while the first is still running, breaking the exactly-once promise. Too long and a genuinely dead worker's job stalls for minutes.

**Poison messages.** Some `Upload` will be malformed — a corrupt file that makes `resize` throw every time. Left alone it returns to the queue, gets picked up, fails, and returns again, forever, burning a worker on each loop. A dead-letter queue is the escape hatch: after N failed attempts the job is moved aside for inspection instead of poisoning the pool.

**Duplicate delivery.** Visibility timeouts and retries make delivery *at-least-once*, not exactly-once. The same `Upload` can legitimately reach two workers. "Resized exactly once" is therefore an effect to engineer, not a gift — make `resize` idempotent by keying the output thumbnail on the upload `id`, so a duplicate overwrites identically and harmlessly.

**Ordering loss.** With many `Resizer` workers pulling concurrently, jobs finish out of submission order. If a user uploads, deletes, then re-uploads the same `id`, the deletes and resizes can race. Competing consumers trade ordering for throughput; if order matters per key, partition the queue so one key always lands on one worker.

## Resilience

Autoscale the worker pool on queue depth: when the backlog grows, add `Resizer` instances; when it drains, remove them. That is the pattern's superpower — throughput is a dial, not a rewrite. Pair a visibility timeout with bounded retries and a dead-letter queue so failures are isolated, not infinite. Keep `resize` idempotent so redelivery is safe. Monitor queue depth and oldest-message age as your primary health signals.

## Pairs well with

**Publish/Subscribe** feeds it: fan an event out to several queues, each drained by its own competing-consumer pool, so every reaction scales independently.

**Transactional Outbox** guarantees the job actually got enqueued — if `Intake` also wrote to a database, the Outbox makes the write and the enqueue atomic.

**Claim Check** keeps the queue light: enqueue a small reference to the upload rather than the bytes, so workers stay cheap to dequeue and the queue stays fast.
