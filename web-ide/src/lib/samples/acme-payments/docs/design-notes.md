# Design Notes

This page records the architectural decisions behind ACME Pay — the ones grounded in how production card-payment platforms actually work — and the places the model deliberately simplifies. The patterns here are industry-standard; the goal of the model is to make the load-bearing decisions explicit rather than to mirror any one provider.

## Decisions grounded in real payment systems

**The seven-state PaymentIntent.** `intents::IntentStatus` is exactly seven states — `RequiresPaymentMethod`, `RequiresConfirmation`, `RequiresAction`, `Processing`, `RequiresCapture`, `Succeeded`, `Canceled` — and nothing else. An earlier draft added an eighth review state; a fraud review doesn't pause the lifecycle (see below), so it was removed.

**The intent is a retry envelope, not a charge wrapper.** A decline returns the intent to `RequiresPaymentMethod` so the *same* intent is re-confirmed, and each confirmation attempt creates a *new* `charges::Charge`. `charges.authorize` mints a fresh charge per attempt, and the intent surfaces the latest.

**Confirm is the branch point.** Create-then-confirm, where `confirm` deterministically transitions to `RequiresAction` (3-D Secure), `Succeeded`, `RequiresCapture` (manual capture), or back to `RequiresPaymentMethod` on decline. `intents::PaymentIntents.confirm` is structured as exactly that branch.

**3-D Secure is an explicit `RequiresAction` branch** driven by an upstream assessment (regulatory SCA mandates + fraud rules + issuer soft declines), with the method-specific step decoupled. ACME Pay carries it as a `context::NetworkChallenge` the cardholder completes via `confirmAction`.

**Fraud review does not stop money.** This corrected the biggest divergence. An outright *block* prevents the charge, but a *review* outcome lets the payment authorise and capture, then flags it for a human to inspect (and refund) afterwards — blocking every flagged payment up front would reject too many good customers. `risk` and `intents::confirm` match this: only `block` halts; `review` flags and proceeds; resolution in `backoffice` is approve-or-refund, not a pre-money gate.

**Idempotency keys are the spine.** A client idempotency key makes a mutating POST effectively exactly-once: the first request's result is cached against the key (including server errors), replayed on retry, and rejected if the key is reused with different parameters. `gateway::Idempotency` models this layer on the object-creating routes (create payment, create refund), where duplicate-prevention matters most.

**A server error is indeterminate; reconcile out of band and emit webhooks.** A 5xx must be treated as "outcome unknown": cache it, reconcile the real result separately, and surface anything reconciliation creates via a webhook rather than by changing the cached response. ACME Pay's `Processing` state, the `batch::SagaSweeper`, and the charge reconciler are the same shape.

**The ledger is an immutable double-entry system of record.** Money movements are appended as immutable `BalanceTransaction` entries; the balance is *derived* by folding the log, never stored as a mutable counter, so past state is always reconstructable. A periodic *clearing* check asserts every account nets to zero and surfaces any discrepancy. `ledger` is built as an append-only `EntryLog`, a derived `Balance`, and a `Clearing` invariant.

**Pending vs available is time-based.** Funds sit in `pending` and become `available` at each balance transaction's `availableOn` time (T+X settlement timing), not on a per-charge event. `ledger::Balance.balance` derives the split from the log against the clock.

**Payout schedule ≠ settlement timing; failures return funds.** A payout reflects the available balance at creation; the schedule controls *when* it is sent, distinct from *when* funds become available; a failed or returned payout restores the funds. `payouts` + `ledger.restore` model this.

**Three key tiers, two isolated modes.** Publishable (client, tokenisation only), secret (full server), restricted (scoped — the recommended tier for staff tooling); live and test have separate keys *and* separate data. `identity` models all three tiers, a `Scope` on restricted keys, and a fixed `Mode`.

**Fulfilment is a server-side webhook consumer.** Fulfilment must run off the `payment_intent.succeeded` webhook, not the client, because the client can disconnect mid-flow. ACME Pay's outbound `webhooks` deliver exactly these events to the merchant's endpoint.

## Deliberate simplifications

- **Refunds are synchronous-final.** Real refunds also settle asynchronously; ACME Pay treats the network acknowledgement as final and omits a refund-settlement state machine. Refunds are off the hot path and rarely contended.
- **Idempotency keys guard creation only.** The edge layer is wired to the object-creating POSTs (payment, refund). State-transition routes (confirm, capture, cancel) rely on the state machine plus the per-charge network keys, which is sufficient for safe retries.
- **Fee and settlement-window computation are black boxes.** `ledger::Balance.net`, `withFee`, and the `availableOn` window are signatures; the arithmetic (interchange, scheme fees, per-network timing) is pricing/policy, not architecture.
- **One acquirer / network abstraction.** A real platform routes across multiple acquirers and networks; the model collapses them into one `context::CardNetwork` boundary.

## On the workflow internals

Production money-movement platforms typically orchestrate these flows through internal workflow/saga systems whose internals aren't public. The saga and reconciliation patterns here (`intents` recovery, the `batch` sweepers) follow the well-documented idempotency-and-reconciliation *principles* rather than any specific internal system.
