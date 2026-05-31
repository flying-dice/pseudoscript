# Edge Cases & Resilience

`Dispatcher.retry` is a single signature in the model, but it hides the most foot-gun-laden pattern in the resilience toolbox. Here's what a senior reviewer pushes on.

## Edge cases & failure modes

**Retry storms.** If every `Postbird` instance retries a struggling `Provider` aggressively, the retries themselves become the outage. The provider hiccups, thousands of clients pile retries on top of the original load, and a 5-second blip turns into a 5-minute meltdown. Backoff alone isn't enough at scale — you also want a *retry budget* (cap retries as a fraction of total requests, e.g. 10%) so the system can't spend more energy retrying than doing real work.

**Idempotency.** The model header calls this out, and it's the sharpest edge. `Provider.deliver` might *succeed* and then the response gets lost — the network drops the ack. `Dispatcher` sees an error, retries, and now the user gets two welcome emails. Email isn't naturally idempotent, so `send` must attach an idempotency key the provider dedupes on. Without it, "retry on transient failure" silently means "occasionally double-send."

**Backoff + jitter, not just backoff.** Exponential backoff alone synchronizes clients: everyone fails at T, everyone retries at T+1s, everyone hammers the provider in the same instant. Jitter (randomizing each delay) is what spreads the herd out. The model names jitter explicitly — drop it and the backoff schedule becomes a synchronized stampede.

**Classifying transient vs permanent.** `SendFailed.transient` is the linchpin, and it's only as good as the classifier that sets it. Mislabel a permanent 400 as transient and you burn the whole budget on a doomed send; mislabel a transient 503 as permanent and you give up on a recoverable one. Ambiguous cases (a timeout — did it arrive?) are the hardest and tie straight back to idempotency.

## Resilience

The model omits the budget, the per-attempt timeout, the idempotency key, and the dead-letter path. Harden it by: bounding each attempt with a timeout (a retry of a hung call is pointless); capping retries with both a per-message budget *and* a global retry budget; attaching an idempotency key so a retried `deliver` can't double-email; and routing a final `GaveUp` to a dead-letter queue for inspection rather than dropping it silently. Cap total backoff so a message can't be stuck retrying for minutes while the `App` waits.

## Pairs well with

- **Circuit breaker** — when retries keep failing, the breaker stops calling `Provider` entirely, so retries can't become a storm. Retry handles blips; the breaker handles sustained failure.
- **Idempotency keys** — the precondition that makes retrying a side-effecting call (sending email) safe.
- **Rate limiter / retry budget** — caps the retry traffic so a recovering provider isn't re-buried.
- **Dead-letter queue** — the home for messages that exhaust their budget and hit `GaveUp`, so nothing is lost.
- **Timeout** — bounds each attempt so a slow call becomes a retriable failure instead of a hang.
