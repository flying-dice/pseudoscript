# Edge Cases & Resilience

A peer-review pass over the design: the specific failure modes the model anticipates, where each is handled, and what would break if it weren't. These are the cases that separate a payments platform from a wrapper around a card-network SDK.

## The authorisation timed out — was the card charged?

**The risk:** the network call to authorise (or capture) times out. The money may or may not have moved, and the call gives no answer. Guess "declined" and you may have charged a card while telling the merchant it failed; guess "succeeded" and you may ship goods for nothing.

**The handling:** `charges::ChargeService.isDefinite` separates a definite decline from an indeterminate timeout. An indeterminate outcome leaves the charge in place and parks the intent in `Processing` (`afterChargeFailure`), never failing it. `batch::SagaSweeper` → `resume` → `reconcileCapture` asks the network the truth and converges — succeeded, failed, or "still unknown, try later." Pinned by `UncertainChargeIsRecovered` and `TimeoutIsReconciled`. **The cardholder is never failed while the charge might be real.**

## The merchant retried the request

**The risk:** networks are unreliable, so clients retry. A naive platform processes the same create twice and ends up with two PaymentIntents — or two charges.

**The handling:** two layers. At the edge, the client's idempotency key makes the POST replay-safe: `gateway::Idempotency.begin` returns the originally created resource on a same-key retry without re-executing (`IdempotentCreateReplays`), and rejects the key if reused with different parameters (`KeyReuseWithDifferentParamsRejected`). At the network, each charge carries its own per-charge key, so a retried authorise or capture call never double-moves money; `capture` also returns the existing receipt (`CaptureIsIdempotent`). A *genuinely new* attempt (a new key after a decline) correctly creates a new charge — the intent is a retry envelope.

## A network callback arrived twice — or never

**The risk:** the network redelivers webhooks and occasionally loses one. A replayed dispute callback could debit a balance twice; a lost capture outcome could strand a charge.

**The handling:** inbound callbacks (`disputes::DisputeService.open` / `resolve`) verify the signature and de-duplicate on the event id (`CallbackLog`) — a replay is acknowledged but changes nothing. And nothing critical depends on a webhook arriving: an indeterminate *charge* converges by reconciliation against the network (`reconcileCapture`), and *settlement* isn't a webhook at all — captured funds mature from pending to available by time (`availableOn`) in the ledger. There is no settlement callback to lose.

## A balance posting was delivered twice

**The risk:** domain events are at-least-once. A `PaymentSucceeded` delivered twice would credit the merchant twice — free money, and a balance that no longer reconciles.

**The handling:** `ledger` owns consumer-side idempotency. Entries are appended to an immutable log via `appendOnce`, idempotent on the entry's `(type, source)` — a duplicate event appends nothing (`CreditIsIdempotent`), and the derived balance is unchanged. The producer uses a transactional outbox and never assumes the consumer de-dupes; the consumer never assumes the producer delivers once. The contract is explicit on both sides.

## Two payouts raced for the same balance

**The risk:** the scheduler runs, a retry fires, and two payouts try to draw the same available balance — paying the merchant twice, or drawing money that isn't there.

**The handling:** `schedule` returns an in-flight payout rather than starting a second (`PayoutIsIdempotent`), and `ledger::Balance.withdraw` appends a debit in one atomic check-and-append that succeeds only if the funds cover it (`PayoutCannotOverdraw`). A rejected or returned transfer appends a reversal (`RejectedPayoutRestoresFunds`). Money never vanishes between the ledger and the bank.

## The card was stolen

**The risk:** a fraudster confirms a payment with a stolen card. By the time the chargeback lands, the goods are gone.

**The handling:** `risk::Engine.screen` runs *before* authorisation on every confirmation. An outright **block** never reaches the network (`BlockedPaymentNeverCharges`). A **review** outcome does *not* halt the payment — it proceeds and is flagged (`ReviewDoesNotHaltPayment`) for an analyst, who can refund it via `backoffice` (`RejectedReviewRefunds`). The screen-then-authorise order is fixed in the disclosed saga; what a review buys is a fast human path to reversal, not a pre-authorisation hold (deliberately avoided, since blocking every flagged payment up front would reject too many good customers).

## A merchant tried to act as another merchant

**The risk:** a merchant passes another merchant's id (or intent id) and reads or charges against an account that isn't theirs.

**The handling:** the merchant is always derived from the authenticated API key, never from the request body (`MerchantIsTakenFromTheKey`). Every read and write is ownership-checked: `intents` refuses a `WrongMerchant`, the backoffice review is merchant-scoped (`ReviewIsMerchantScoped`), and a non-owner is told "not found", never that the resource exists.

## The refund to the network failed

**The risk:** a refund debits the merchant's balance, but the network reversal fails — the merchant is out the money with no reversal at the card.

**The handling:** `refunds::RefundService.create` reverses at the network *first* and debits the balance only on success, via `RefundSucceeded`. A failed reversal records the refund `Failed` and moves no balance (`FailedRefundDoesNotDebit`). The balance only ever moves to match money that actually moved.

## A merchant's webhook endpoint was down

**The risk:** the merchant's server is briefly unavailable when an event fires. Drop the event and the merchant's records drift from the platform's.

**The handling:** `webhooks::Dispatcher` records every event for delivery (de-duplicated on the event id) and retries with backoff until it succeeds or is exhausted (`FailedDeliveryRetries`), driven by `batch::WebhookRedeliverer`. A transient outage costs latency, not data.

## The deliberate scope cuts

Two simplifications are choices, not oversights:

- **Refunds are synchronous-final.** A refund completes when the network acknowledges; there is no asynchronous refund-settlement state mirroring capture's. Refunds are off the hot path and rarely contended, so the extra state machine isn't worth its weight.
- **Fee computation is a black box.** `ledger::Balance.net` and `withFee` compute the platform's cut, but *how* the fee is calculated (interchange, scheme fees, the merchant's pricing tier) is pricing policy, not architecture — its inputs and outputs are modelled, its arithmetic is not.

Everything else in the failure surface above is handled in a disclosed body with a feature pinning the behaviour — which is the test of the model: you could rebuild the platform from it, and the corners would still be correct.
