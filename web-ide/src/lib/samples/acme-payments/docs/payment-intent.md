# The PaymentIntent Lifecycle

The PaymentIntent is the state machine at the centre of ACME Pay. Every payment is one intent moving through a fixed set of seven states, and the saga in `intents::PaymentIntents` is the only thing allowed to move it. The states are not cosmetic: each is a durable progress marker, so a crash leaves the intent at its last persisted step and the sweeper resumes it from there. Crucially, the intent is a **retry envelope** — a decline returns it to `requires_payment_method` so the *same* intent is re-confirmed, and each attempt creates a new `charges::Charge`.

## The states

```
RequiresConfirmation ──confirm──▶ (screen for fraud)
                                       │
                              block ───┴─── allow / review
                                │              │
                                ▼              ▼
                            Canceled        authorise ──3DS?──▶ RequiresAction ──auth──▶ (continue)
                                                 │                     │
                                                 │                  fail ▼
                                                 ▼              RequiresPaymentMethod ◀── decline
                              automatic ─────────┴───────── manual
                                  │                            │
                                  ▼                            ▼
                              Succeeded ◀──capture──────  RequiresCapture

   (timeout anywhere on the charge path) ──▶ Processing ──sweeper reconciles──▶ Succeeded | RequiresPaymentMethod
```

- **`RequiresConfirmation`** — created, awaiting a payment method and confirmation.
- **`RequiresAction`** — the issuer demanded 3-D Secure; parked until the cardholder authenticates.
- **`RequiresCapture`** — authorised, awaiting the merchant's manual capture.
- **`Processing`** — a charge whose outcome is unknown (a network timeout), under reconciliation.
- **`Succeeded`** / **`Canceled`** — terminal.
- **`RequiresPaymentMethod`** — the recoverable failure state a decline falls back to, making the intent retryable.

These are the seven canonical statuses — no more. An earlier draft added a `Review` state; it was removed, because (see below) a fraud review doesn't pause the state machine.

## The confirm saga

`confirm` is the spine. Read it as a sequence of guards, each of which can divert the intent, with money moving only past the last one:

1. **Ownership.** `getFor(merchant, intent)` refuses an intent that belongs to another merchant — the merchant comes from the authenticated key, so one merchant can never confirm another's payment.
2. **Confirmable.** The intent must be awaiting confirmation (or retrying from `requires_payment_method`).
3. **Instrument.** `vault::Instruments.instrumentFor` resolves the payment method to a network instrument. A method that isn't the merchant's is `InvalidInstrument`. The PAN never appears.
4. **Fraud.** `risk::Engine.screen` returns block / review / allow. **Block** cancels the intent and raises `PaymentFailed` — *the network is never called.* **Review** does *not* stop the payment: it flags the intent for a human and falls through to authorisation. **Allow** falls through too. (Only a block stops money; a review is a post-hoc flag.)
5. **Authorise.** `charges::ChargeService.authorize` mints a charge for this attempt and places the hold. Its outcome is the branch point: a 3-D Secure requirement parks the intent in `RequiresAction`; otherwise the saga continues to capture-or-hold.
6. **Capture.** Automatic capture calls `finalize` (capture now, succeed, raise `PaymentSucceeded`); manual capture parks the intent in `RequiresCapture`.

The crucial ordering: **screen, then authorise, then capture** — fixed in the disclosed body, not reorderable by a caller. A blocked payment never authorises; everything else proceeds and is flagged if risky.

## 3-D Secure

When the issuer demands authentication, `charges` returns `NeedsAction` carrying a challenge, and the intent parks in `RequiresAction`. The cardholder completes the challenge directly with their bank, then calls `confirmAction` (authorised by the intent's client secret, not the merchant's key). That resumes exactly where `confirm` left off: complete the challenge at the network, then capture-or-hold. A merchant can't skip the challenge, and a cardholder can't capture — the two paths meet only at `afterAuthorized`. The authentication step is decoupled from the core lifecycle (so other methods could slot in); ACME Pay carries the one card case, a network challenge.

## Recovery — the timeout that must not lie

The failure that justifies the whole machine is a network timeout during authorise or capture: the money may or may not have moved, and ACME Pay cannot tell from the call alone. `afterChargeFailure` encodes the rule:

- A **definite** decline resets the intent to `RequiresPaymentMethod` and raises `PaymentFailed`. Safe — the network told us nothing happened.
- An **indeterminate** failure parks the intent in `Processing` and returns `Uncertain`. The saga does *not* fail it.

The `batch::SagaSweeper` later calls `resume`, which asks `charges` to reconcile the charge against the network. A charge the network confirms drives the intent to `Succeeded`; one the network *definitively* denies abandons it; an outcome still unknown is left for the next sweep. This is the standard treatment of an indeterminate server error: cache the result, reconcile out of band, and emit a webhook for what reconciliation creates. The invariant, pinned by `UncertainChargeIsRecovered`: **the cardholder is never failed while the charge might be real.** Every recovery branch is idempotent.
