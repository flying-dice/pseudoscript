# Money Movement & Idempotency

`charges` is the only context that moves money at the card network, and it is built around one assumption: **every network call can be retried, and some will time out with the outcome unknown.** Idempotency — at the API edge and at every network call — is what makes that assumption safe. Idempotency keys exist precisely to resolve the problem of an ambiguous failure (you can't tell whether the first request took effect); this model leans on them throughout.

## Two layers of idempotency

**At the edge.** Every object-creating POST carries a client `Idempotency-Key`. `gateway::Idempotency.begin` reserves the key on the first request and lets it proceed; a retry with the same key replays the cached result (the created resource) *without re-executing*, and a retry with *different parameters* under the same key is rejected (`KeyConflict`). A cached server error is replayed too, so an indeterminate failure stays indeterminate for the client until ACME Pay reconciles it. This is wired into create-payment and create-refund — the two places duplicate-prevention matters most.

**At the network.** Every call through `charges::Gateway` carries an idempotency key scoped to the *charge* and operation (`keyFor(charge, "authorize")`, `"capture"`, `"refund"`, `"void"`). The network is contractually idempotent on that key, so any single call can be retried exactly once and yield the same answer.

## One charge per attempt — the retry envelope

A PaymentIntent is a retry envelope: a decline returns it to `requires_payment_method`, and each confirmation attempt creates a **new** `Charge` (`ChargeStore.nextId`). The intent surfaces the latest. This is why network idempotency keys are scoped to the charge, not the intent — two genuine attempts must reach the network as two distinct operations, while a *retry of one attempt* (same edge key) never re-enters the saga at all.

`capture` is idempotent within an attempt: it checks `receiptFor(intent)` first and returns the existing receipt if the capture already succeeded (`CaptureIsIdempotent`).

## The three failure modes

`CardNetwork` fails in three ways, and the difference between them is the entire point:

| Network error | Meaning | Charge response |
|---|---|---|
| `Declined` | The issuer said no. **Definite.** | Mark `Failed` (and void any hold on a capture failure). |
| `RateLimited` | Try again shortly. Retryable under the same key. | Surface; the caller retries. |
| `Unreachable` | We never got an answer. **Indeterminate.** | Leave the charge in place for reconciliation. |

`isDefinite` is the guard that separates them. On an authorise failure, `onAuthError` marks the charge `Failed` *only* if the error is definite; an indeterminate timeout leaves it `Authorizing`. On a capture failure, `onCaptureError` fails the charge only if definite — because an indeterminate capture **may actually have settled**, and treating it as failed would lose money the cardholder was charged.

## Reconciliation — converging the unknown

`reconcileCapture` resolves an indeterminate charge. If no receipt exists yet, it asks the network the truth:

- `lookupCapture` returns a receipt → the capture *did* settle → record `Captured`.
- `captureDefinitelyMissing` returns `Ok` → the network *definitively* reports no capture → record `Failed`.
- Otherwise (still unreachable) → leave it; the next sweep tries again.

This is what `definitelyFailed` exposes to the intent saga: `Ok` only when the ledger *definitively* records the attempt as failed. A still-`Authorizing` charge returns `Err`, so the saga never abandons an intent on uncertainty (`TimeoutIsReconciled`). This is the standard way to handle an indeterminate server error — cache the result, reconcile out of band, surface new objects via webhooks.

## Where settlement lives

Note what `charges` does *not* do: it captures funds, but it does not track *settlement* (clearing). In this model, captured funds sit in the merchant's **pending** balance and become **available** by the passage of time (`available_on`), derived in `ledger`, not by a per-charge settlement webhook. The only inbound network callbacks ACME Pay must react to are **disputes** (`disputes::DisputeService`), which are signed and de-duplicated like any webhook. Keeping settlement out of `charges` is what lets the charge engine stay a pure money-mover.

## Why this shape

The recurring move is: **make the operation idempotent, then retry freely.** A per-charge key gives idempotent network calls; an edge key gives idempotent (replayable) HTTP; folding an immutable ledger gives idempotent accounting. Once every step is idempotent, the timeout stops being special — you reconcile and retry until the truth converges, and no path can move money twice.
